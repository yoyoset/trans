import clsx from "clsx";
import type { ButtonHTMLAttributes } from "react";

type Variant = "primary" | "secondary" | "ghost" | "danger";

const VARIANTS: Record<Variant, string> = {
  primary: "bg-emerald-600 text-white hover:bg-emerald-500",
  secondary: "bg-zinc-800 text-zinc-100 hover:bg-zinc-700",
  ghost: "text-zinc-300 hover:bg-zinc-800",
  danger: "bg-rose-700 text-white hover:bg-rose-600",
};

export function Button({
  variant = "secondary",
  className,
  type = "button",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant }) {
  return (
    <button
      type={type}
      className={clsx(
        "inline-flex items-center justify-center gap-1.5 rounded-md px-3 py-1.5 text-sm font-medium transition",
        "disabled:cursor-not-allowed disabled:opacity-50",
        VARIANTS[variant],
        className,
      )}
      {...props}
    />
  );
}
