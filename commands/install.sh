#!/bin/bash

# ATHA Info Command
# Show package information (Official Repo + AUR Fallback)

source "$(dirname "$0")/lib/colors.sh"
source "$(dirname "$0")/lib/utils.sh"
source "$(dirname "$0")/lib/validators.sh"

PKG=$1

[ -z "$PKG" ] && die "Masukkan nama package"

validate_package_name "$PKG"

print_info "atha - safety and workflow layer for pacman"
print_processing "Fetching package info: $PKG"
log "Info requested for package: $PKG"

echo ""

if pacman -Si "$PKG" 2>/dev/null; then
    log "Info success for official package: $PKG"
    exit 0
fi

if pacman -Qi "$PKG" 2>/dev/null; then
    print_warning "Package not found in sync database, showing installed local info:"
    echo ""
    pacman -Qi "$PKG"
    log "Info success for local package: $PKG"
    exit 0
fi

if command -v curl >/dev/null 2>&1; then
    aur_check="$(curl -s "https://aur.archlinux.org/rpc/v5/info/${PKG}" 2>/dev/null)"
    if echo "$aur_check" | grep -q "\"Name\":\"${PKG}\""; then
        print_section "AUR Package Information"
        print_info "Package found in AUR: https://aur.archlinux.org/packages/${PKG}"
        if command -v jq >/dev/null 2>&1; then
            echo "$aur_check" | jq -r '.results[0] | " Name           : " + .Name + "\n Version        : " + .Version + "\n Description    : " + (.Description // "N/A") + "\n URL            : " + (.URL // "N/A") + "\n Maintainer     : " + (.Maintainer // "Orphaned")'
        fi
        log "Info success for AUR package: $PKG"
        exit 0
    fi
fi

log "Info failed for package: $PKG"
die "Package not found in official repositories, local DB, or AUR: $PKG"