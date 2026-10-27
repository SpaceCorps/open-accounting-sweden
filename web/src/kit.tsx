import {
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  DatePicker,
  Input,
  TimeInput,
  Label,
} from "@spacecorps/components/ui";
import { toast } from "@spacecorps/components";
import { useId, type ReactNode } from "react";
import { ApiError } from "./api";

export function PageHeader({
  title,
  description,
  actions,
}: {
  title: string;
  description?: string;
  actions?: ReactNode;
}) {
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

export function Kpi({
  label,
  value,
  hint,
  badge,
}: {
  label: string;
  value: ReactNode;
  hint?: string;
  badge?: ReactNode;
}) {
  return (
    <Card>
      <CardHeader>
        <CardDescription>{label}</CardDescription>
        <CardTitle className="oas-kpi-value">{value}</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="oas-muted oas-kpi-hint">
          {badge}
          {hint}
        </div>
      </CardContent>
    </Card>
  );
}

export function Section({
  title,
  description,
  actions,
  children,
}: {
  title: string;
  description?: string;
  actions?: ReactNode;
  children: ReactNode;
}) {
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

export function Field({
  label,
  children,
  hint,
  htmlFor,
}: {
  label: string;
  children: ReactNode;
  hint?: string;
  htmlFor?: string;
}) {
  return (
    <div className="oas-field">
      <Label htmlFor={htmlFor}>{label}</Label>
      {children}
      {hint ? <span className="oas-muted oas-hint">{hint}</span> : null}
    </div>
  );
}

export function TextField(props: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  type?: string;
  placeholder?: string;
  hint?: string;
  autoComplete?: string;
  /** Form field name; password managers use it with autoComplete to recognise sign-in forms. */
  name?: string;
  /** For names, codes and usernames: no spellcheck, autocorrect or capitalisation. */
  verbatim?: boolean;
}) {
  const generated = useId();
  const id = props.name ?? generated;
  if (props.type === "date") {
    // The library's picker shows yyyy-mm-dd whatever the browser locale, with one Tab stop.
    return (
      <Field label={props.label} hint={props.hint} htmlFor={id}>
        <DatePicker id={id} name={props.name} value={props.value} onChange={props.onChange} />
      </Field>
    );
  }
  if (props.type === "time") {
    // 24-hour HH:MM in any browser locale; typing 930 gives 09:30, arrows step a quarter hour.
    return (
      <Field label={props.label} hint={props.hint} htmlFor={id}>
        <TimeInput
          id={id}
          name={props.name}
          value={props.value}
          onChange={props.onChange}
          step={15}
          suggestionStep={30}
        />
      </Field>
    );
  }
  return (
    <Field label={props.label} hint={props.hint} htmlFor={id}>
      <Input
        id={id}
        name={props.name}
        type={props.type ?? "text"}
        value={props.value}
        placeholder={props.placeholder}
        autoComplete={props.autoComplete}
        {...(props.verbatim
          ? { spellCheck: false, autoCorrect: "off", autoCapitalize: "none" }
          : {})}
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

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
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

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@spacecorps/components/ui";

export interface VatCode {
  code: string;
  label: string;
  base_box: string | null;
}

/** Optional VAT code for the net line of a voucher. An empty value means no code. */
export function VatCodeSelect(props: {
  value: string;
  onChange: (v: string) => void;
  codes: VatCode[];
}) {
  return (
    <Select
      value={props.value || "none"}
      onValueChange={(v) => props.onChange(v === "none" ? "" : v)}
    >
      <SelectTrigger aria-label="VAT code">
        <SelectValue placeholder="VAT code" />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="none">No VAT code</SelectItem>
        {props.codes.map((c) => (
          <SelectItem key={c.code} value={c.code}>
            {c.code}: {c.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
