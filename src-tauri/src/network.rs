//! IPv4 TCP disconnect for the profile executable.
//!
//! Rows are chosen without calling Windows. The adapter then finds process ids by exe name and
//! deletes the selected TCBs.

use std::collections::HashSet;
use std::ffi::c_void;

use windows::Win32::Foundation::{CloseHandle, ERROR_INSUFFICIENT_BUFFER, HANDLE};
use windows::Win32::Globalization::{CompareStringOrdinal, CSTR_EQUAL};
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, SetTcpEntry, MIB_TCPROW_LH, MIB_TCPROW_LH_0, MIB_TCP_STATE_DELETE_TCB,
    MIB_TCP_STATE_ESTAB, MIB_TCP_STATE_SYN_RCVD, MIB_TCP_STATE_SYN_SENT, TCP_TABLE_OWNER_PID_ALL,
};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};

/// `AF_INET`. `GetExtendedTcpTable` takes the address family as `u32`.
const AF_INET: u32 = 2;
const TCP_TABLE_ATTEMPTS: usize = 3;

/// One IPv4 row of `TCP_TABLE_OWNER_PID_ALL`. Addresses and ports stay in the table's byte order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnedTcpRow {
    pub state: u32,
    pub local_addr: u32,
    pub local_port: u32,
    pub remote_addr: u32,
    pub remote_port: u32,
    pub owning_pid: u32,
}

/// Endpoint copied into `SetTcpEntry` without swapping address or port bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TcpEndpoint {
    pub local_addr: u32,
    pub local_port: u32,
    pub remote_addr: u32,
    pub remote_port: u32,
}

/// Rows whose owner is in `pids` and whose state is established, SYN sent, or SYN received.
pub fn rows_to_disconnect(rows: &[OwnedTcpRow], pids: &HashSet<u32>) -> Vec<TcpEndpoint> {
    rows.iter()
        .filter(|row| pids.contains(&row.owning_pid) && is_live_connection(row.state))
        .map(|row| TcpEndpoint {
            local_addr: row.local_addr,
            local_port: row.local_port,
            remote_addr: row.remote_addr,
            remote_port: row.remote_port,
        })
        .collect()
}

fn is_live_connection(state: u32) -> bool {
    let state = state as i32;
    state == MIB_TCP_STATE_ESTAB.0
        || state == MIB_TCP_STATE_SYN_SENT.0
        || state == MIB_TCP_STATE_SYN_RCVD.0
}

/// `szExeFile` matches the profile process name. An empty profile name matches nothing.
pub fn exe_name_matches(exe_file: &str, process_name: &str) -> bool {
    if process_name.is_empty() {
        return false;
    }
    let exe_file: Vec<u16> = exe_file.encode_utf16().collect();
    let process_name: Vec<u16> = process_name.encode_utf16().collect();
    // Both slices remain valid for the call; Windows supplies the Unicode case mapping.
    unsafe { CompareStringOrdinal(&exe_file, &process_name, true) == CSTR_EQUAL }
}

/// Breaks the IPv4 TCP connections of every process whose exe name matches `process_name`.
/// A missing process, a table error, or a failed row is logged; the call still returns.
pub fn disconnect(process_name: &str) {
    let pids = match process_ids(process_name) {
        Ok(pids) => pids,
        Err(error) => {
            eprintln!("Не удалось получить список процессов: {error}");
            return;
        }
    };
    if pids.is_empty() {
        return;
    }
    let rows = match tcp_owner_rows() {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!("Не удалось прочитать таблицу TCP: {error}");
            return;
        }
    };
    for endpoint in rows_to_disconnect(&rows, &pids) {
        let status = delete_tcb(&endpoint);
        if status != 0 {
            eprintln!("Не удалось оборвать TCP-соединение: код {status}");
        }
    }
}

fn process_ids(process_name: &str) -> Result<HashSet<u32>, String> {
    if process_name.is_empty() {
        return Ok(HashSet::new());
    }
    let snapshot = ProcessSnapshot(
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }
            .map_err(|error| error.to_string())?,
    );
    Ok(unsafe { collect_process_ids(snapshot.0, process_name) })
}

struct ProcessSnapshot(HANDLE);

impl Drop for ProcessSnapshot {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

unsafe fn collect_process_ids(snapshot: HANDLE, process_name: &str) -> HashSet<u32> {
    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..PROCESSENTRY32W::default()
    };
    if unsafe { Process32FirstW(snapshot, &mut entry) }.is_err() {
        return HashSet::new();
    }
    let mut pids = HashSet::new();
    loop {
        if exe_name_matches(&exe_file_name(&entry.szExeFile), process_name) {
            pids.insert(entry.th32ProcessID);
        }
        entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
        if unsafe { Process32NextW(snapshot, &mut entry) }.is_err() {
            break;
        }
    }
    pids
}

fn exe_file_name(exe_file: &[u16]) -> String {
    let end = exe_file
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(exe_file.len());
    String::from_utf16_lossy(&exe_file[..end])
}

fn tcp_owner_rows() -> Result<Vec<OwnedTcpRow>, String> {
    let mut capacity = 0u32;
    for _attempt in 0..TCP_TABLE_ATTEMPTS {
        let mut words = vec![0u32; capacity.div_ceil(4) as usize];
        let mut size = (words.len() * size_of::<u32>()) as u32;
        let pointer = (!words.is_empty()).then_some(words.as_mut_ptr().cast::<c_void>());
        let status = unsafe {
            GetExtendedTcpTable(
                pointer,
                &mut size,
                false,
                AF_INET,
                TCP_TABLE_OWNER_PID_ALL,
                0,
            )
        };
        if status == 0 {
            return Ok(parse_owner_table(&words));
        }
        if status != ERROR_INSUFFICIENT_BUFFER.0 {
            return Err(format!("код {status}"));
        }
        capacity = size;
    }
    Err("буфер не хватил за три попытки".into())
}

fn parse_owner_table(words: &[u32]) -> Vec<OwnedTcpRow> {
    let Some((&count, rows)) = words.split_first() else {
        return Vec::new();
    };
    rows.chunks_exact(6)
        .take(count as usize)
        .map(|row| OwnedTcpRow {
            state: row[0],
            local_addr: row[1],
            local_port: row[2],
            remote_addr: row[3],
            remote_port: row[4],
            owning_pid: row[5],
        })
        .collect()
}

fn delete_tcb(endpoint: &TcpEndpoint) -> u32 {
    let row = MIB_TCPROW_LH {
        Anonymous: MIB_TCPROW_LH_0 {
            dwState: MIB_TCP_STATE_DELETE_TCB.0 as u32,
        },
        dwLocalAddr: endpoint.local_addr,
        dwLocalPort: endpoint.local_port,
        dwRemoteAddr: endpoint.remote_addr,
        dwRemotePort: endpoint.remote_port,
    };
    unsafe { SetTcpEntry(&row) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::NetworkManagement::IpHelper::{
        MIB_TCP_STATE_CLOSE_WAIT, MIB_TCP_STATE_LISTEN,
    };

    fn owned(pid: u32, state: i32, local_port: u32, remote_port: u32) -> OwnedTcpRow {
        OwnedTcpRow {
            state: state as u32,
            local_addr: 0x0100_007f,
            local_port,
            remote_addr: 0x0808_0808,
            remote_port,
            owning_pid: pid,
        }
    }

    fn selected(rows: &[OwnedTcpRow], pids: &[u32]) -> Vec<TcpEndpoint> {
        rows_to_disconnect(rows, &pids.iter().copied().collect())
    }

    #[test]
    fn copies_an_established_connection_without_swapping_bytes() {
        let local_port = 0x1234_5678;
        let remote_port = 0x00ab_cd00;
        let row = owned(42, MIB_TCP_STATE_ESTAB.0, local_port, remote_port);

        let endpoints = selected(&[row], &[42]);

        assert_eq!(
            endpoints,
            [TcpEndpoint {
                local_addr: row.local_addr,
                local_port,
                remote_addr: row.remote_addr,
                remote_port,
            }]
        );
        assert_ne!(endpoints[0].local_port, local_port.swap_bytes());
        assert_ne!(endpoints[0].local_addr, row.local_addr.swap_bytes());
        assert_ne!(endpoints[0].remote_port, remote_port.swap_bytes());
    }

    #[test]
    fn selects_syn_sent_and_syn_received() {
        let rows = [
            owned(7, MIB_TCP_STATE_SYN_SENT.0, 1, 2),
            owned(7, MIB_TCP_STATE_SYN_RCVD.0, 3, 4),
        ];

        let endpoints = selected(&rows, &[7]);

        assert_eq!(endpoints.len(), 2);
        assert_eq!(endpoints[0].local_port, 1);
        assert_eq!(endpoints[1].local_port, 3);
    }

    #[test]
    fn skips_a_foreign_pid() {
        assert!(selected(&[owned(7, MIB_TCP_STATE_ESTAB.0, 1, 2)], &[8]).is_empty());
    }

    #[test]
    fn skips_a_listening_socket() {
        assert!(selected(&[owned(7, MIB_TCP_STATE_LISTEN.0, 80, 0)], &[7]).is_empty());
    }

    #[test]
    fn skips_other_tcp_states() {
        assert!(selected(&[owned(7, MIB_TCP_STATE_CLOSE_WAIT.0, 1, 2)], &[7]).is_empty());
    }

    #[test]
    fn matches_exe_name_ignoring_case() {
        let wide: Vec<u16> = "Game.EXE"
            .encode_utf16()
            .chain(std::iter::once(0))
            .chain(std::iter::once(u16::from(b'x')))
            .collect();

        assert!(exe_name_matches(&exe_file_name(&wide), "game.exe"));
        assert!(!exe_name_matches(&exe_file_name(&wide), "other.exe"));
        assert!(!exe_name_matches("Game.EXE", ""));
        assert!(!exe_name_matches("", ""));
    }

    #[test]
    fn matches_unicode_exe_names_ignoring_case() {
        assert!(exe_name_matches("Игра.EXE", "игра.exe"));
        assert!(exe_name_matches("Élite.exe", "élite.EXE"));
        assert!(!exe_name_matches("Игра.exe", "другая.exe"));
    }

    #[test]
    fn empty_process_name_selects_no_pids() {
        assert!(process_ids("").unwrap().is_empty());
    }
}
