import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import type { ButtonHTMLAttributes } from "react";
import { cn } from "@/lib/utils";

const buttonVariants = cva("inline-flex min-h-10 items-center justify-center gap-2 rounded-lg px-4 text-center text-sm font-medium transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-sky-600 disabled:pointer-events-none disabled:opacity-50", { variants: { variant: { default: "bg-sky-700 text-white hover:bg-sky-800", outline: "border border-slate-300 bg-white text-slate-800 hover:bg-slate-50", ghost: "text-slate-700 hover:bg-slate-100", destructive: "bg-red-700 text-white hover:bg-red-800" }, size: { default: "min-h-10", sm: "min-h-8 px-3 text-xs", lg: "min-h-12 px-5" } }, defaultVariants: { variant: "default", size: "default" } });

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement>, VariantProps<typeof buttonVariants> { asChild?: boolean }
export function Button({ className, variant, size, asChild = false, ...props }: ButtonProps) {
  const Comp = asChild ? Slot : "button";
  return <Comp className={cn(buttonVariants({ variant, size }), className)} {...props} />;
}
