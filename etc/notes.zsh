#!/usr/bin/env false

# Generate a string cache:
(set -eux; version="1.4.2-1"
git -C ~/all-crosscode-versions checkout ${version}
output=~/crosscode/localize-me-string-caches/str-cache-${version}.json
lmt-jsontr --gamedir ~/all-crosscode-versions --string-cache-file ${output} save_cache
gzip ${output})

# Perform a migration:
(set -eux; version_from="1.4.1-2" version_to="1.4.2-1"; for cclocale in es_ES pt_BR ru_RU uk_UA vi_VN; do
crosslocale create-project tmp --translation-locale ${cclocale} ~/crosscode/crosscode-crosslocale-scans/scan-${version_from}.json --splitter monolithic-file
crosslocale import tmp -f po ~/crosscode/crosscode-localization-data/po/${cclocale}/components
crosslocale export tmp -f lm-tr-pack -o old.pack.json --remove-untranslated
\rm -rf tmp
lmt-packfile --string-cache <(zcat ~/crosscode/localize-me-string-caches/str-cache-${version_to}.json.gz) \
  migrate ~/lmt_migration_${version_from}_${version_to}.json old.pack.json new.pack.json --mark-unknown
\rm -rf ~/crosscode/crosscode-localization-data/po/${cclocale}/components/*.po(N)
crosslocale create-project tmp --translation-locale ${cclocale} ~/crosscode/crosscode-crosslocale-scans/scan-${version_to}.json --splitter monolithic-file
crosslocale import tmp -f lm-tr-pack new.pack.json
crosslocale export tmp -f po -o ~/crosscode/crosscode-localization-data/po/${cclocale}/components --splitter notabenoid-chapters
\rm -rf tmp new.pack.json old.pack.json
done)

# Import from crosscode-ru:
(set -eux;
crosslocale create-project tmp --translation-locale ru_RU ~/crosscode/crosscode-ru/assets/ru-translation-tool/scan.json --splitter monolithic-file
crosslocale import tmp -f cc-ru-chapter-fragments ~/crosscode/crosscode-ru/assets/ru-translation-tool/chapter-fragments
crosslocale export tmp -f po -o ~/crosscode/crosscode-localization-data/po/ru_RU/components --splitter notabenoid-chapters
\rm -rf tmp)

# Generate the patched scan database:
node ~/crosscode/crosscode-ru/tool/dist/headless-scan.js --gameAssetsDir ~/all-crosscode-versions/assets --ccloaderDir ~/crosscode/ccloader3 --outputFile ~/crosscode/crosscode-localization-data/scan.json

# Re-import all translation projects:
(set -eux; for cclocale in es_ES pt_BR ru_RU uk_UA vi_VN; do
crosslocale create-project tmp --translation-locale ${cclocale} ~/crosscode/crosscode-localization-data/scan.json --splitter monolithic-file
crosslocale import tmp -f po ~/crosscode/crosscode-localization-data/po/${cclocale}/components
\rm -rf ~/crosscode/crosscode-localization-data/po/${cclocale}/components/*.po(N)
crosslocale export tmp -f po -o ~/crosscode/crosscode-localization-data/po/${cclocale}/components --splitter notabenoid-chapters
\rm -rf tmp
done)

# Add a language:
(set -eux;
crosslocale create-project tmp --translation-locale tr_TR ~/crosscode/crosscode-ru/assets/ru-translation-tool/scan.json --splitter monolithic-file
crosslocale export tmp -f po -o ~/crosscode/crosscode-localization-data/po/tr_TR/components --splitter notabenoid-chapters
\rm -rf tmp)
