#!/usr/bin/env bash
# Checks every guide link on the Learn page (assets/learn.json): prints the HTTP status of each URL
# (GET, redirects followed) and, for YouTube, the status of YouTube's oEmbed lookup too, because a
# removed or private video still answers its watch page with 200 while oEmbed says 401/404.
# Exit code 1 when any link fails.
#
# Usage: launcher/tools/check-learn-links.sh [learn.json]
# Needs curl and jq. Light on CPU (one request at a time), so no srl-cpu needed.
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
json=${1:-$here/assets/learn.json}
ua='Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130 Safari/537.36'
fail=0
n=0
printf '%-4s %-7s %-22s %s\n' HTTP oEmbed guide url
while IFS=$'\t' read -r id url; do
  n=$((n + 1))
  code=$(curl -s -L -A "$ua" -o /dev/null -w '%{http_code}' --max-time 30 "$url" || true)
  oembed=-
  case "$url" in
    https://www.youtube.com/watch\?v=* | https://youtu.be/* | https://www.youtube.com/playlist\?list=*)
      oembed=$(curl -s -G -o /dev/null -w '%{http_code}' --max-time 30 \
        --data-urlencode "url=$url" --data-urlencode format=json https://www.youtube.com/oembed || true)
      ;;
  esac
  printf '%-4s %-7s %-22s %s\n' "$code" "$oembed" "$id" "$url"
  if [[ ! $code =~ ^2 ]] || { [ "$oembed" != - ] && [[ ! $oembed =~ ^2 ]]; }; then
    fail=$((fail + 1))
  fi
done < <(jq -r '.topics[].guides[] | [.id, .url] | @tsv' "$json")
echo "$n links, $fail failed"
[ "$fail" -eq 0 ]
