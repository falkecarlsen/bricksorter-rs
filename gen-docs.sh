# comment out figure tags for docs
header="Bricksorter docs ($(date --iso-8601) @falkecarlsen)"

# remove figures and add pandoc specific header
cp README.md README-tmp.md
sed -i -E 's/!\[[^]]*\]\([^)]+\)//g' README-tmp.md
sed -i "1s/^/%$header\n/" README-tmp.md

# gen pdf
pandoc -V geometry:a4paper,margin=2cm README-tmp.md -o README-no-fig.pdf

# cleanup 
rm README-tmp.md
