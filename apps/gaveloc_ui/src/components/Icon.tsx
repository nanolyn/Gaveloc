import { FC, CSSProperties } from 'react';
import {
  ArrowClockwise,
  CaretDown,
  CaretRight,
  CaretUp,
  CheckCircle,
  Check,
  CornersOut,
  Download,
  EyeSlash,
  Eye,
  FolderOpen,
  Folder,
  GameController,
  GearSix,
  House,
  Info,
  Key,
  LockOpen,
  Lock,
  Minus,
  PencilSimple,
  Play,
  Plus,
  ShieldCheck,
  ShieldWarning,
  SignIn,
  SignOut,
  CircleNotch,
  Trash,
  UserCircle,
  UserPlus,
  User,
  WarningCircle,
  Warning,
  XCircle,
  X,
  Article,
  ChatText,
  CalendarStar,
  Megaphone,
  Sparkle,
  Wrench,
} from '@phosphor-icons/react';
import type { Icon as PhosphorIcon, IconWeight } from '@phosphor-icons/react';

const icons = {
  'arrow-clockwise': ArrowClockwise,
  'caret-down': CaretDown,
  'caret-right': CaretRight,
  'caret-up': CaretUp,
  'check-circle': CheckCircle,
  'check': Check,
  'corners-out': CornersOut,
  'download': Download,
  'eye-slash': EyeSlash,
  'eye': Eye,
  'folder-open': FolderOpen,
  'folder': Folder,
  'game-controller': GameController,
  'gear': GearSix,
  'house': House,
  'info': Info,
  'key': Key,
  'lock-open': LockOpen,
  'lock': Lock,
  'minus': Minus,
  'pencil': PencilSimple,
  'play': Play,
  'plus': Plus,
  'shield-check': ShieldCheck,
  'shield-warning': ShieldWarning,
  'sign-in': SignIn,
  'sign-out': SignOut,
  'spinner': CircleNotch,
  'trash': Trash,
  'user-circle': UserCircle,
  'user-plus': UserPlus,
  'user': User,
  'warning-circle': WarningCircle,
  'warning': Warning,
  'x-circle': XCircle,
  'x': X,
  'article': Article,
  'chat-text': ChatText,
  'calendar-star': CalendarStar,
  'megaphone': Megaphone,
  'sparkle': Sparkle,
  'wrench': Wrench,
} as const;

export type IconName = keyof typeof icons;

interface IconProps {
  name: IconName;
  size?: number | string;
  weight?: IconWeight;
  className?: string;
  style?: CSSProperties;
}

export const Icon: FC<IconProps> = ({
  name,
  size = 24,
  weight = 'regular',
  className = '',
  style,
  ...props
}) => {
  const IconComponent = icons[name] as PhosphorIcon;

  if (!IconComponent) {
    console.warn(`Icon "${name}" not found`);
    return null;
  }

  return (
    <IconComponent
      size={typeof size === 'string' ? parseInt(size) : size}
      weight={weight}
      className={`icon icon-${name} ${className}`}
      style={{ flexShrink: 0, ...style }}
      {...props}
    />
  );
};

export default Icon;
