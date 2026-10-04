import { Home, Info, Settings, type LucideIcon } from "lucide-react";

export type NavItem = {
  to: string;
  label: string;
  icon: LucideIcon;
  end?: boolean;
};

export const navItems: NavItem[] = [
  { to: "/", label: "Home", icon: Home, end: true },
  { to: "/settings", label: "Settings", icon: Settings },
  { to: "/about", label: "About", icon: Info },
];
