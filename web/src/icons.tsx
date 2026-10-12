import type { ComponentType, SVGProps } from "react";
import { Building } from "pixelarticons/react/Building";
import { Grid3x3 } from "pixelarticons/react/Grid3x3";
import { Wallet as PixelWallet } from "pixelarticons/react/Wallet";
import { Coins } from "pixelarticons/react/Coins";
import { Leaf } from "pixelarticons/react/Leaf";
import { Map } from "pixelarticons/react/Map";
import { FileText as PixelFileText } from "pixelarticons/react/FileText";
import { Folder } from "pixelarticons/react/Folder";
import { Package as PixelPackage } from "pixelarticons/react/Package";
import { Logout } from "pixelarticons/react/Logout";
import { SlidersHorizontal } from "pixelarticons/react/SlidersHorizontal";
import { Refresh } from "pixelarticons/react/Refresh";
import { ArrowRight as PixelArrowRight } from "pixelarticons/react/ArrowRight";
import { ArrowUp } from "pixelarticons/react/ArrowUp";
import { ArrowDown } from "pixelarticons/react/ArrowDown";
import { ChevronRight as PixelChevronRight } from "pixelarticons/react/ChevronRight";
import { Flag as PixelFlag } from "pixelarticons/react/Flag";
import { Check as PixelCheck } from "pixelarticons/react/Check";
import { Close } from "pixelarticons/react/Close";
import { Download as PixelDownload } from "pixelarticons/react/Download";
import { Search as PixelSearch } from "pixelarticons/react/Search";
import { CircleInfo } from "pixelarticons/react/CircleInfo";
import { Loader } from "pixelarticons/react/Loader";
import { Archive as PixelArchive } from "pixelarticons/react/Archive";
import { Copy as PixelCopy } from "pixelarticons/react/Copy";
import { Key } from "pixelarticons/react/Key";
import { Plus as PixelPlus } from "pixelarticons/react/Plus";
import { Power as PixelPower } from "pixelarticons/react/Power";
import { Shield as PixelShield } from "pixelarticons/react/Shield";
import { Trash } from "pixelarticons/react/Trash";
import { Circle } from "pixelarticons/react/Circle";

type IconProps = SVGProps<SVGSVGElement> & { size?: number | string };
function icon(Component: ComponentType<SVGProps<SVGSVGElement>>) {
  return function PixelIcon({
    size = 24,
    strokeWidth: _strokeWidth,
    className,
    ...props
  }: IconProps) {
    return (
      <Component
        width={size}
        height={size}
        focusable="false"
        shapeRendering="crispEdges"
        aria-hidden="true"
        className={`pixel-icon ${className || ""}`}
        {...props}
      />
    );
  };
}
export const Building2 = icon(Building);
export const LayoutDashboard = icon(Grid3x3);
export const Wallet = icon(PixelWallet);
export const Coin = icon(Coins);
export const Sprout = icon(Leaf);
export const GitBranch = icon(Map);
export const FileText = icon(PixelFileText);
export const FolderOpen = icon(Folder);
export const Package = icon(PixelPackage);
export const LogOut = icon(Logout);
export const Settings2 = icon(SlidersHorizontal);
export const RefreshCw = icon(Refresh);
export const ArrowRight = icon(PixelArrowRight);
export const ArrowUpRight = icon(ArrowUp);
export const ArrowDownLeft = icon(ArrowDown);
export const ChevronRight = icon(PixelChevronRight);
export const Flag = icon(PixelFlag);
export const Check = icon(PixelCheck);
export const X = icon(Close);
export const Download = icon(PixelDownload);
export const Search = icon(PixelSearch);
export const AlertCircle = icon(CircleInfo);
export const LoaderCircle = icon(Loader);
export const Archive = icon(PixelArchive);
export const Copy = icon(PixelCopy);
export const KeyRound = icon(Key);
export const Plus = icon(PixelPlus);
export const Power = icon(PixelPower);
export const Shield = icon(PixelShield);
export const Trash2 = icon(Trash);
export const CheckCircle2 = icon(PixelCheck);
export const XCircle = icon(Close);
export const CircleDot = icon(Circle);
