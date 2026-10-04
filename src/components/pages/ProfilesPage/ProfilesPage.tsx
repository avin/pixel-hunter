import { Pencil, Plus, Trash2 } from "lucide-react";
import { useState } from "react";
import { Link } from "react-router-dom";
import { profilePath } from "@/components/app/AppRouter/paths";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { useListProfilesQuery } from "@/store/api";
import type { Profile } from "@/store/types";
import { DeleteProfileDialog } from "./DeleteProfileDialog/DeleteProfileDialog";
import { ProfileDialog } from "./ProfileDialog/ProfileDialog";

export function ProfilesPage() {
  const { data: profiles, isLoading, isError, refetch } = useListProfilesQuery();
  const [editedProfile, setEditedProfile] = useState<{ profile?: Profile }>();
  const [profileToDelete, setProfileToDelete] = useState<Profile>();

  const openCreateDialog = () => setEditedProfile({});

  function renderContent() {
    if (isLoading) {
      return (
        <div role="status" aria-label="Загрузка профилей" className="flex flex-col gap-2">
          <Skeleton className="h-10" />
          <Skeleton className="h-10" />
          <Skeleton className="h-10" />
        </div>
      );
    }
    if (isError || !profiles) {
      return (
        <div role="alert" className="flex flex-col items-start gap-3">
          <p className="text-sm">Не удалось загрузить профили.</p>
          <Button variant="outline" onClick={() => refetch()}>
            Повторить
          </Button>
        </div>
      );
    }
    if (profiles.length === 0) {
      return (
        <div className="flex flex-col items-center gap-3 rounded-xl border border-dashed px-6 py-12 text-center">
          <h2 className="font-heading text-base font-medium">Профилей пока нет</h2>
          <p className="max-w-sm text-sm text-muted-foreground">
            Профиль привязан к процессу игры и объединяет отслеживания для неё.
          </p>
          <Button onClick={openCreateDialog}>
            <Plus />
            Создать профиль
          </Button>
        </div>
      );
    }
    return (
      <>
        <div className="flex justify-end">
          <Button onClick={openCreateDialog}>
            <Plus />
            Создать профиль
          </Button>
        </div>
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Имя</TableHead>
              <TableHead>Процесс</TableHead>
              <TableHead className="w-0">
                <span className="sr-only">Действия</span>
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {profiles.map((profile) => (
              <TableRow key={profile.id}>
                <TableCell>
                  <Link
                    to={profilePath(profile.id)}
                    className="font-medium underline-offset-4 outline-none hover:underline focus-visible:underline focus-visible:ring-3 focus-visible:ring-ring/50"
                  >
                    {profile.name}
                  </Link>
                </TableCell>
                <TableCell className="text-muted-foreground">
                  {profile.processName}
                </TableCell>
                <TableCell>
                  <div className="flex justify-end gap-1">
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      aria-label={`Изменить профиль «${profile.name}»`}
                      onClick={() => setEditedProfile({ profile })}
                    >
                      <Pencil />
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      aria-label={`Удалить профиль «${profile.name}»`}
                      onClick={() => setProfileToDelete(profile)}
                    >
                      <Trash2 />
                    </Button>
                  </div>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </>
    );
  }

  return (
    <div className="flex flex-col gap-4 p-6">
      {renderContent()}
      {editedProfile && (
        <ProfileDialog
          profile={editedProfile.profile}
          onClose={() => setEditedProfile(undefined)}
        />
      )}
      {profileToDelete && (
        <DeleteProfileDialog
          profile={profileToDelete}
          onClose={() => setProfileToDelete(undefined)}
        />
      )}
    </div>
  );
}
