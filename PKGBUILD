# Maintainer: Bangkah <...>
pkgname=atha
pkgver=3.1.1
pkgrel=1
pkgdesc="A safety and workflow layer for pacman"
arch=('x86_64')
url="https://github.com/Bangkah/Atha"
license=('MIT')
depends=('pacman' 'sudo' 'git')
source=("$pkgname-$pkgver::https://github.com/Bangkah/Atha/releases/download/v$pkgver/atha-$CARCH-linux")
sha256sums=('ad5d632362a032f001054742244f2e6f16750bdcf6066ff5833a2218ccac8d14')

package() {
    cd "$srcdir/$_pkgname-$pkgver"
    
    # Instal biner utama hasil build Cargo
    install -Dm755 "target/release/atha" "$pkgdir/usr/bin/atha"
    
    # Salin file lisensi agar sesuai dengan standar Arch Linux
    if [ -f "LICENSE" ]; then
        install -Dm644 "LICENSE" "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
    elif [ -f "LICENSE.md" ]; then
        install -Dm644 "LICENSE.md" "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
    elif [ -f "LICENSE-MIT" ]; then
        install -Dm644 "LICENSE-MIT" "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
    fi
}