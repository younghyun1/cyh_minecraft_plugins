#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
gradle build
cp build/libs/StartingEquipment-*.jar .
echo "Done: $(ls StartingEquipment-*.jar)"
