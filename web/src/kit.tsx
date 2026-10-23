import {
  Button, Card, CardContent, CardDescription, CardHeader, CardTitle, Input, Label,
} from "@spacecorps/components/ui";
import { toast } from "@spacecorps/components";
import type { ReactNode } from "react";
import { ApiError } from "./api";

export function PageHeader({ title, description, actions }: { title: string; description?: string; actions?: ReactNode }) {
  return (
    <div className="oas-page-header">
      <div>
        <h1 className="oas-title">{title}</h1>
        {description ? <p className="oas-muted">{description}</p> : null}
      </div>
      <div className="oas-actions">{actions}</div>
    </div>
  );
}

export function Kpi({ label, value, hint, badge }: { label: string; value: ReactNode; hint?: string; badge?: ReactNode }) {
  return (
    <Card>
      <CardHeader>
        <CardDescription>{label}</CardDescription>
        <CardTitle className="oas-kpi-value">{value}</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="oas-muted oas-kpi-hint">{badge}{hint}</div>
      </CardContent>
    </Card>
  );
}

export function Section({ title, description, actions, children }: { title: string; description?: string; actions?: ReactNode; children: ReactNode }) {
  return (
    <Card>
      <CardHeader>
        <div className="oas-section-head">
          <div>
            <CardTitle>{title}</CardTitle>
            {description ? <CardDescription>{description}</CardDescription> : null}
          </div>
          <div className="oas-actions">{actions}</div>
        </div>
      </CardHeader>
      <CardContent>{children}</CardContent>
    </Card>
  );
}

export function Field({ label, children, hint }: { label: string; children: ReactNode; hint?: string }) {
  return (
    <div className="oas-field">
      <Label>{label}</Label>
      {children}
      {hint ? <span className="oas-muted oas-hint">{hint}</span> : null}
    </div>
  );
}

export function TextField(props: { label: string; value: string; onChange: (v: string) => void; type?: string; placeholder?: string; hint?: string; autoComplete?: string }) {
  return (
    <Field label={props.label} hint={props.hint}>
      <Input
        type={props.type ?? "text"}
        value={props.value}
        placeholder={props.placeholder}
        autoComplete={props.autoComplete}
        onChange={(e) => props.onChange(e.target.value)}
      />
    </Field>
  );
}

export function notifyError(e: unknown) {
  const message = e instanceof ApiError || e instanceof Error ? e.message : "Something went wrong";
  toast({ title: "Error", description: message, variant: "destructive" });
}

export function notifyOk(title: string, description?: string) {
  toast({ title, description });
}

export function ErrorNote({ message }: { message: string | null }) {
  return message ? <p className="oas-error">{message}</p> : null;
}

export function EmptyNote({ children }: { children: ReactNode }) {
  return <p className="oas-muted oas-empty">{children}</p>;
}

export { Button };
export const money = (s: string) => s;
export const hoursText = (h: number) => `${h.toFixed(2)} h`;

import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription,
  AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
} from "@spacecorps/components/ui";

export function ConfirmDialog(props: {
  open: boolean;
  title: string;
  description: string;
  confirmLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  return (
    <AlertDialog open={props.open} onOpenChange={(o) => !o && props.onCancel()}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{props.title}</AlertDialogTitle>
          <AlertDialogDescription>{props.description}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <AlertDialogAction onClick={props.onConfirm}>{props.confirmLabel}</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}

import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@spacecorps/components/ui";

export interface VatCode { code: string; label: string; base_box: string | null }

/** Optional VAT code for the net line of a voucher. An empty value means no code. */
export function VatCodeSelect(props: { value: string; onChange: (v: string) => void; codes: VatCode[] }) {
  return (
    <Select value={props.value || "none"} onValueChange={(v) => props.onChange(v === "none" ? "" : v)}>
      <SelectTrigger aria-label="VAT code"><SelectValue placeholder="VAT code" /></SelectTrigger>
      <SelectContent>
        <SelectItem value="none">No VAT code</SelectItem>
        {props.codes.map((c) => (
          <SelectItem key={c.code} value={c.code}>{c.code}: {c.label}</SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
