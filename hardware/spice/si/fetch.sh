#!/bin/sh
# Fetch the vendor IBIS models into models/ (not kept in the repository:
# they are the manufacturers' files). Each goes into its own directory.
# Prints what it could not get; the simulation then uses its generic driver.
set -u
cd "$(dirname "$0")"
get() {  # directory, file, URL
    [ -s "models/$1/$2" ] && return 0
    mkdir -p "models/$1"
    if ! curl -sSfL -A curl -m 60 -o "models/$1/$2" "$3"; then
        rm -f "models/$1/$2"; echo "not fetched: $2 from $3"; return 1
    fi
}
# SN74LVC8T245: Texas Instruments literature number SCEM494
get ti-lvc8t245 scem494.ibs https://www.ti.com/lit/mo/scem494/scem494.ibs
# SN74LVC245A: Texas Instruments literature number SCAM006 (a zip holding one .ibs)
if get ti-lvc245a scam006.zip https://www.ti.com/lit/zip/scam006; then
    ls models/ti-lvc245a/*.ibs >/dev/null 2>&1 || unzip -q -o -j models/ti-lvc245a/scam006.zip '*.ibs' -d models/ti-lvc245a \
        || echo "not unpacked: scam006.zip"
fi
# ECP5: Lattice publishes its IBIS file as a 6.9 MB archive behind the product
# page. It is not fetched; the FPGA pad is a generic model (see driver notes).
echo "not fetched: ECP5 IBIS (Lattice FPGA-MD-02094); generic LVCMOS33 8 mA pad used"
