#!/bin/bash

# ATHA Install Command
# Install packages

source "$(dirname "$0")/lib/colors.sh"
source "$(dirname "$0")/lib/utils.sh"
source "$(dirname "$0")/lib/validators.sh"
source "$(dirname "$0")/lib/progress.sh"

validate_command pacman

current_build_dir=""
cleanup_on_exit() {
    if [ -n "$current_build_dir" ] && [ -d "$current_build_dir" ]; then
        rm -rf "$current_build_dir"
    fi
}
trap cleanup_on_exit EXIT

dry_run=0
plan_only=0
auto_yes=0
packages=()

while [ $# -gt 0 ]; do
    case "$1" in
        --dry-run)
            dry_run=1
            ;;
        --plan)
            plan_only=1
            ;;
        --yes)
            auto_yes=1
            ;;
        --*)
            die "Unknown option: $1"
            ;;
        *)
            packages+=("$1")
            ;;
    esac
    shift
done

if [ ${#packages[@]} -eq 0 ]; then
    die "Masukkan nama package"
fi

for pkg in "${packages[@]}"; do
    validate_package_name "$pkg"
done

echo ""
print_info "atha - safety and workflow layer for pacman"
print_processing "Starting installation flow"
echo ""

plan_lines=()
actionable_count=0
official_count=0
aur_count=0
skip_count=0
official_targets=()
aur_targets=()
skipped_targets=()

for pkg in "${packages[@]}"; do
    if pacman -Qi "$pkg" &>/dev/null; then
        plan_lines+=("$pkg|skip|installed|already installed")
        skipped_targets+=("$pkg")
        skip_count=$((skip_count + 1))
        continue
    fi

    actionable_count=$((actionable_count + 1))
    if pacman -Si "$pkg" &>/dev/null; then
        plan_lines+=("$pkg|install|official|found in official repositories")
        official_targets+=("$pkg")
        official_count=$((official_count + 1))
    else
        plan_lines+=("$pkg|install|aur|not found in official repositories")
        aur_targets+=("$pkg")
        aur_count=$((aur_count + 1))
    fi
done

if [ "$plan_only" -eq 1 ]; then
    print_section "PLAN: Decision Analysis"
    for line in "${plan_lines[@]}"; do
        IFS='|' read -r pkg action source reason <<< "$line"
        if [ "$action" = "skip" ]; then
            echo "  - $pkg -> skip"
            echo "    reason: $reason"
        elif [ "$source" = "official" ]; then
            echo "  - $pkg -> install from official"
            echo "    reason: $reason"
        else
            echo "  - $pkg -> install from AUR"
            echo "    reason: $reason, fallback to AUR workflow"
        fi
    done

    if [ ${#official_targets[@]} -gt 0 ]; then
        print_section "PLAN: Official Transaction Simulation"
        official_txn="$(pacman -S --print --print-format '%n|%s' "${official_targets[@]}" 2>/dev/null || true)"
        if [ -n "$official_txn" ]; then
            official_txn_count="$(printf "%s\n" "$official_txn" | sed '/^\s*$/d' | wc -l | tr -d ' ')"
            official_bytes="$(printf "%s\n" "$official_txn" | awk -F'|' '{sum+=$2} END {print sum+0}')"
            print_info "Packages in transaction (requested + dependencies): $official_txn_count"
            print_info "Estimated download size: $(format_bytes "$official_bytes")"
            while IFS='|' read -r txn_pkg txn_size; do
                [ -z "$txn_pkg" ] && continue
                marker="dependency"
                for req_pkg in "${official_targets[@]}"; do
                    if [ "$txn_pkg" = "$req_pkg" ]; then
                        marker="requested"
                        break
                    fi
                done
                echo "  - $txn_pkg ($(format_bytes "$txn_size"), $marker)"
            done <<< "$official_txn"
        else
            print_warning "Unable to simulate official transaction"
        fi
    fi

    if [ ${#aur_targets[@]} -gt 0 ]; then
        print_section "PLAN: AUR Reachability"
        for pkg in "${aur_targets[@]}"; do
            if command -v git >/dev/null 2>&1 && git ls-remote --exit-code "https://aur.archlinux.org/${pkg}.git" >/dev/null 2>&1; then
                print_success "$pkg repository reachable"
            else
                print_warning "$pkg repository not reachable or git unavailable"
            fi
        done
    fi

    print_section "PLAN: Summary"
    print_info "install=$actionable_count official=$official_count aur=$aur_count skip=$skip_count"

    for line in "${plan_lines[@]}"; do
        IFS='|' read -r pkg action source reason <<< "$line"
        if [ "$action" = "skip" ]; then
            record_history "install" "$pkg" "$source" "skipped" "plan:$reason"
        else
            record_history "install" "$pkg" "$source" "planned" "plan:$reason"
        fi
    done
    echo ""
    print_success "Plan completed (no changes applied)"
    exit 0
fi

if [ "$dry_run" -eq 1 ]; then
    log "Install dry-run requested for packages: ${packages[*]}"
    print_section "DRY-RUN: Execution Simulation"
    print_info "No package changes will be applied"

    for line in "${plan_lines[@]}"; do
        IFS='|' read -r pkg action source reason <<< "$line"
        if [ "$action" = "skip" ]; then
            record_history "install" "$pkg" "$source" "skipped" "dry-run:$reason"
            print_warning "$pkg -> already installed (skip)"
        else
            record_history "install" "$pkg" "$source" "planned" "dry-run:$reason"
            if [ "$source" = "official" ]; then
                print_info "$pkg -> would execute: sudo pacman -S $pkg"
            else
                if [ "$auto_yes" -eq 1 ]; then
                    print_info "$pkg -> would execute: git clone https://aur.archlinux.org/$pkg.git && makepkg -si --noconfirm"
                else
                    print_info "$pkg -> would execute: git clone https://aur.archlinux.org/$pkg.git && makepkg -si"
                fi
            fi
        fi
    done

    echo ""
    print_info "Execution summary: install=$actionable_count official=$official_count aur=$aur_count skip=$skip_count"
    print_success "Dry-run completed (no changes applied)"
    exit 0
fi

print_section "Execution Plan"
if [ ${#official_targets[@]} -gt 0 ]; then
    print_info "Official repo targets: ${official_targets[*]}"
fi
if [ ${#aur_targets[@]} -gt 0 ]; then
    print_info "AUR targets: ${aur_targets[*]}"
fi
if [ ${#skipped_targets[@]} -gt 0 ]; then
    print_warning "Skipped (already installed): ${skipped_targets[*]}"
fi
echo ""
print_info "Summary: install=$actionable_count official=$official_count aur=$aur_count skip=$skip_count"

if [ "$actionable_count" -eq 0 ]; then
    print_warning "Nothing to install"
    exit 0
fi

if [ "$auto_yes" -ne 1 ]; then
    read -p "Proceed with installation? (y/N) " -n 1 -r
    echo ""
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        print_info "Operation cancelled"
        log "Install cancelled by user for packages: ${packages[*]}"
        exit 0
    fi
fi

for line in "${plan_lines[@]}"; do
    IFS='|' read -r pkg action source reason <<< "$line"
    if [ "$action" = "skip" ]; then
        log "Skip install (already installed): $pkg"
        record_history "install" "$pkg" "$source" "skipped" "already-installed"
    fi
done

if [ ${#official_targets[@]} -gt 0 ]; then
    print_processing "Installing official packages: ${official_targets[*]}"
    log "Executing batch install for official packages: ${official_targets[*]}"
    progress_bar

    validate_sudo

    pacman_flags=("-S" "--needed")
    if [ "$auto_yes" -eq 1 ]; then
        pacman_flags+=("--noconfirm")
    fi

    if sudo pacman "${pacman_flags[@]}" "${official_targets[@]}"; then
        for pkg in "${official_targets[@]}"; do
            print_success "$pkg installed"
            log "Install success: $pkg"
            record_history "install" "$pkg" "official" "success" ""
        done
    else
        for pkg in "${official_targets[@]}"; do
            log "Install failed: $pkg"
            record_history "install" "$pkg" "official" "failed" "pacman batch failure"
        done
        die "Failed to install official packages"
    fi
fi

if [ ${#aur_targets[@]} -gt 0 ]; then
    validate_command git
    validate_command makepkg

    for pkg in "${aur_targets[@]}"; do
        print_processing "Installing $pkg from AUR"
        log "Source detected for $pkg: aur"
        progress_bar

        build_dir="$(mktemp -d "/tmp/atha-${pkg}-XXXXXX")"
        current_build_dir="$build_dir"
        log "AUR build directory: $build_dir"

        if ! git clone "https://aur.archlinux.org/${pkg}.git" "$build_dir/$pkg"; then
            record_history "install" "$pkg" "aur" "failed" "git clone failed"
            die "Failed to clone AUR package: $pkg"
        fi

        # Sebelumnya --noconfirm dipaksa terus, jadi prompt dependency/provider/
        # conflict dari makepkg ikut ter-auto-approve walau user tidak pernah
        # menyertakan --yes. Sekarang perilakunya disamakan dengan jalur official:
        # --noconfirm cuma ditambahkan kalau auto_yes aktif.
        (
            cd "$build_dir/$pkg" || exit 1
            makepkg_flags=("-si")
            if [ "$auto_yes" -eq 1 ]; then
                makepkg_flags+=("--noconfirm")
            fi
            makepkg "${makepkg_flags[@]}"
        )
        rc=$?

        rm -rf "$build_dir"
        current_build_dir=""

        if [ $rc -eq 0 ]; then
            print_success "$pkg installed"
            log "Install success: $pkg"
            record_history "install" "$pkg" "aur" "success" ""
        else
            log "Install failed: $pkg"
            record_history "install" "$pkg" "aur" "failed" "exit=$rc"
            die "Failed install $pkg"
        fi
    done
fi

echo ""
print_success "Install process completed"