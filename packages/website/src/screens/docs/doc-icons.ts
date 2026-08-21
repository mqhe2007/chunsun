import {
  Bell,
  BookOpen,
  Bug,
  Check,
  Compass,
  Database,
  Download,
  Folder,
  Hash,
  Key,
  Keyboard,
  LayoutGrid,
  Lightbulb,
  Link,
  Lock,
  Play,
  Settings,
  Shield,
  SquareCheck,
  TriangleAlert,
  User,
  Wrench,
  Zap,
  type LucideIcon,
} from "@lucide/vue";

export const docIcons = {
  zap: Zap,
  compass: Compass,
  download: Download,
  folder: Folder,
  link: Link,
  play: Play,
  lightbulb: Lightbulb,
  check: Check,
  "square-check": SquareCheck,
  database: Database,
  wrench: Wrench,
  bug: Bug,
  "book-open": BookOpen,
  "triangle-alert": TriangleAlert,
  settings: Settings,
  bell: Bell,
  user: User,
  shield: Shield,
  lock: Lock,
  keyboard: Keyboard,
  "layout-grid": LayoutGrid,
  hash: Hash,
  key: Key,
} as const satisfies Record<string, LucideIcon>;

export type DocIconName = keyof typeof docIcons;

export function resolveDocIcon(name: string): LucideIcon {
  return (docIcons as Record<string, LucideIcon>)[name] ?? Folder;
}
