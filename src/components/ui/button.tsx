import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import type { ButtonHTMLAttributes } from "react";
import { cn } from "@/lib/utils";

const buttonVariants = cva("inline-flex min-h-10 items-center justify-center gap-2 rounded-lg px-4 text-center text-sm font-medium transition-all focus:outline-none disabled:pointer-events-none disabled:opacity-50", { variants: { variant: { default: "bg-gradient-to-r from-primary to-[#ff6699] text-white shadow-sm shadow-pink-200 hover:from-primary-hover hover:to-[#e85b84] active:from-primary-active active:to-[#d1436f]", outline: "border border-slate-300 bg-white text-slate-800 hover:border-primary/40 hover:bg-selected/60 active:bg-selected", ghost: "text-slate-700 hover:bg-selected/80 hover:text-primary active:bg-selected", destructive: "bg-red-600 text-white hover:bg-red-700 active:bg-red-800" }, size: { default: "min-h-10", sm: "min-h-8 px-3 text-xs", lg: "min-h-12 px-5" } }, defaultVariants: { variant: "default", size: "default" } });

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement>, VariantProps<typeof buttonVariants> { asChild?: boolean }
export function Button({ className, variant, size, asChild = false, ...props }: ButtonProps) {
  const Comp = asChild ? Slot : "button";
  return <Comp className={cn(buttonVariants({ variant, size }), className)} {...props} />;
}
