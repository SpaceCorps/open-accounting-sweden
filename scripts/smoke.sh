#!/usr/bin/env bash
# End-to-end smoke test against a throwaway local server. Needs: cargo, curl, sqlite3, python3.
# Usage: scripts/smoke.sh   (builds the workspace, starts a server on 127.0.0.1:18999, checks the rules)
set -u
cd "$(dirname "$0")/.."
cargo build --workspace -q || exit 1
T=$(mktemp -d)
ADMIN="smoke-admin-$(head -c 16 /dev/urandom | od -An -tx1 | tr -d ' \n')"
AGENT="smoke-agent-$(head -c 16 /dev/urandom | od -An -tx1 | tr -d ' \n')"
OAS_TOKEN=$ADMIN OAS_AGENT_TOKEN=$AGENT OAS_DB=$T/s.db OAS_BIND=127.0.0.1:18999 ./target/debug/oas-server 2>$T/server.log &
SERVER=$!
trap 'kill $SERVER 2>/dev/null; rm -rf "$T"' EXIT
for _ in $(seq 1 30); do curl -s -o /dev/null http://127.0.0.1:18999/health && break; sleep 1; done
O=./target/debug/oas; U=http://127.0.0.1:18999
FAIL=0
ok()   { echo "ok    $1"; }
bad()  { echo "FAIL  $1"; FAIL=1; }
check(){ if eval "$2" >/dev/null 2>&1; then ok "$1"; else bad "$1"; fi; }
admin(){ OAS_CONFIG_DIR=$T/adm "$O" "$@" </dev/null; }
agent(){ OAS_CONFIG_DIR=$T/agt "$O" "$@" </dev/null; }
OAS_CONFIG_DIR=$T/adm $O auth login $U --token "$ADMIN" </dev/null >/dev/null
OAS_CONFIG_DIR=$T/agt $O auth login $U --token "$AGENT" </dev/null >/dev/null

check "unauthenticated API is refused" '[ "$(curl -s -o /dev/null -w %{http_code} $U/api/projects)" = 401 ]'
admin voucher propose --date 2026-07-15 --description "AWS July" --debit 6540=1000@RCEU --debit 2645=250 --credit 2614=250 --credit 1930=1000 >/dev/null
check "agent cannot approve a draft" '! agent voucher approve 1'
check "admin approves a draft" 'admin voucher approve 1'
check "unbalanced voucher is rejected" '! admin voucher propose --date 2026-07-16 --description x --debit 6540=10 --credit 1930=9'
check "unknown VAT code is rejected" '! admin voucher propose --date 2026-07-16 --description x --debit 6540=10@NOPE --credit 1930=10'
check "ledger verifies" 'admin ledger verify'
check "posted entry cannot be edited" '! sqlite3 $T/s.db "UPDATE entry SET debit = 1"'
check "posted voucher cannot be deleted" '! sqlite3 $T/s.db "DELETE FROM voucher"'
check "VAT box 21 holds the reverse charge base" 'admin --json vat report --quarter 2026-Q3 | python3 -c "import sys,json; d=json.load(sys.stdin); b={x[\"box\"]:x[\"amount\"] for x in d[\"boxes\"]}; sys.exit(0 if b[\"21\"]==\"1000.00\" and b[\"30\"]==\"250.00\" and b[\"48\"]==\"250.00\" else 1)"'
check "a box marked not used fails the report if an amount lands in it" 'admin vat box-not-used 21 >/dev/null && admin --json vat report --quarter 2026-Q3 | python3 -c "import sys,json; d=json.load(sys.stdin); sys.exit(0 if any(c[\"check\"].startswith(\"no amount in a box marked not used\") and not c[\"ok\"] for c in d[\"checks\"]) else 1)" && admin vat box-not-used 21 --used >/dev/null'
check "a voucher can be corrected once" 'admin voucher correct 1 && ! admin voucher correct 1'
check "VAT report is not ready while boxes are unverified" 'admin --json vat report --quarter 2026-Q3 | python3 -c "import sys,json; sys.exit(0 if json.load(sys.stdin)[\"ready_to_file\"] is False else 1)"'

printf 'receipt' > $T/r.txt
printf 'Datum;Text;Belopp\n2026-10-21;GITHUB INC;-40,00\n' > $T/b.csv
check "bank CSV imports" 'admin bank import $T/b.csv'
check "bank CSV re-import adds nothing" 'admin --json bank import $T/b.csv | python3 -c "import sys,json; sys.exit(0 if json.load(sys.stdin)[\"imported\"]==0 else 1)"'
check "receipt uploads and duplicates are detected" 'admin doc add $T/r.txt --vendor GitHub --amount 40 --date 2026-10-21 >/dev/null && admin --json doc add $T/r.txt | python3 -c "import sys,json; sys.exit(0 if json.load(sys.stdin)[\"document\"][\"duplicate\"] else 1)"'
check "a receipt matches its bank line" 'admin --json match list | python3 -c "import sys,json; sys.exit(0 if len(json.load(sys.stdin)[\"matches\"])==1 else 1)"'
check "agent cannot confirm a match" '! agent match confirm 1'
check "receipts cannot be edited" '! sqlite3 $T/s.db "UPDATE document SET filename = \"x\""'

check "an agent cannot add an employee" '! agent employee add Anna --identity 200407204567'
check "a bad personnummer is rejected" '! admin employee add Bad --identity 198503141234'
check "a rate can be marked not used and then blocks nobody" 'admin payroll not-used contrib_youth && admin payroll rates | grep -A8 "key: contrib_youth$" | grep -q "not_used: true"'
YOUNG=$(python3 -c "
b='20040720456'; d=b[2:]; t=0
for i,c in enumerate(d):
    v=int(c)*(2 if i%2==0 else 1); t+= v-9 if v>9 else v
print(b+str((10-t%10)%10))")
admin employee add "Young Person" --identity "$YOUNG" >/dev/null
check "a run needing a not-used rate is refused" 'admin payroll run --period 2026-10 --pay-date 2026-10-25 --line 1=1000:10% 2>&1 | grep -q "marked not used"'
check "AGI needs an approved run" '! admin payroll agi 1'

echo
if [ $FAIL = 0 ]; then echo "all checks passed"; else echo "some checks FAILED"; fi
exit $FAIL
