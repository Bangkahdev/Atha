# Maintainer: Bangkah <Muhammad Dhiyaul Atha>
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
    cd "$srcdir"
    
    # Karena sumbernya adalah file biner tunggal, langsung pasang ke /usr/bin/atha
    install -Dm755 "$pkgname-$pkgver" "$pkgdir/usr/bin/atha"
    
    # Salin lisensi jika tersedia di repo/workspace
    if [ -f "$startdir/LICENSE" ]; then
        install -Dm644 "$startdir/LICENSE" "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
    elif [ -f "$startdir/LICENSE.md" ]; then
        install -Dm644 "$startdir/LICENSE.md" "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
    fi
}