#!/usr/bin/env bash
# Starts a throwaway server with a month of realistic demo data, for screenshots and demos.
# Usage: scripts/demo.sh [port]   (default 18990; needs a web build in web/dist for the panel)
# Sign in to the web panel as demo / demo-password-123. Ctrl+C stops it and deletes the data.
set -eu
cd "$(dirname "$0")/.."
PORT=${1:-18990}
cargo build --workspace -q
T=$(mktemp -d)
ADMIN="demo-admin-$(head -c 16 /dev/urandom | od -An -tx1 | tr -d ' \n')"
OAS_TOKEN=$ADMIN OAS_DB=$T/demo.db OAS_DATA_DIR=$T/data OAS_BIND=127.0.0.1:$PORT OAS_WEB_DIR=$PWD/web/dist \
  ./target/debug/oas-server 2>$T/server.log &
SERVER=$!
trap 'kill $SERVER 2>/dev/null; wait $SERVER 2>/dev/null; rm -rf "$T"' EXIT
U=http://127.0.0.1:$PORT
for _ in $(seq 1 30); do curl -s -o /dev/null $U/health && break; sleep 1; done
export OAS_CONFIG_DIR=$T/cfg
o() { ./target/debug/oas "$@" </dev/null >/dev/null; }
o auth login $U --token "$ADMIN"
echo demo-password-123 | ./target/debug/oas user add demo --role admin >/dev/null

o company set "Norrsken Konsult AB" --org-number 556026-9986 --address "Sveavägen 12\n111 57 Stockholm" --f-skatt
o customer add "Acme Logistik AB" --org-number 556677-8899 --address "Hamngatan 4\n411 06 Göteborg" --email ekonomi@acme.example
o customer add "Fjällbacka Design AB" --address "Storgatan 9\n457 40 Fjällbacka"
o project add "Acme Logistik" --alias acme --client "Acme Logistik AB" --hourly 1150
o project add "Fjällbacka webbplats" --alias fjb --client "Fjällbacka Design AB" --fixed 48000
o project add "Intern" --alias intern
o activity add building --message "Building: "
o activity add strategizing --message "Strategy: "

# Weekdays in October 2026 up to the 23rd.
for d in 01 02 05 06 07 08 09 12 13 14 15 16 19 20 21 22 23; do
  o time log --activity building --project acme --date 2026-10-$d --from 08:30 --to 12:00 --message "Route planning API"
  o time log --activity meeting --project acme --date 2026-10-$d --from 13:00 --to 13:30
  case $d in 05|12|19) o time log --activity strategizing --project fjb --date 2026-10-$d --from 14:00 --to 16:30 --message "Design review" ;; esac
  case $d in 09|16|23) o time log --activity building --project intern --date 2026-10-$d --from 15:00 --to 17:00 --message "Bookkeeping" ;; esac
done

o invoice from-time --customer "Acme Logistik AB" --project acme --from 2026-10-01 --to 2026-10-09
o invoice issue 1
o invoice create --customer "Fjällbacka Design AB" --line "Webbplats, första delbetalning|1|st|24000"

o voucher propose --date 2026-10-02 --description "AWS September" --counterparty "Amazon Web Services" --debit 6540=1250@RCEU --debit 2645=312.50 --credit 2614=312.50 --credit 1930=1250
o voucher approve 2
o voucher propose --date 2026-10-14 --description "Hörlurar" --counterparty "Clas Ohlson" --debit 5410=320 --debit 2641=80 --credit 1930=400

printf 'Datum;Text;Belopp\n2026-10-02;AWS EMEA;-1250,00\n2026-10-14;CLAS OHLSON;-400,00\n2026-10-20;GITHUB INC;-48,00\n2026-10-21;TELIA SVERIGE;-499,00\n' > $T/bank.csv
o bank import $T/bank.csv
printf 'receipt' > $T/github.txt
o doc add $T/github.txt --vendor GitHub --amount 48 --date 2026-10-20
printf 'receipt 2' > $T/telia.txt
o doc add $T/telia.txt --vendor Telia --amount 499 --date 2026-10-21

o employee add "Astrid Lindqvist" --identity 198503141239
o employee add "Johan Berg" --identity 199206112345
o payroll run --period 2026-10 --pay-date 2026-10-25 --line 1=46000:31% --line 2=38500:29%

echo "Demo running at $U  (sign in as demo / demo-password-123, Ctrl+C to stop)"
wait $SERVER
