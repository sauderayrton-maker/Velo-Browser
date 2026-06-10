PREFIX  ?= /usr/local
BINDIR   = $(DESTDIR)$(PREFIX)/bin
APPDIR   = $(DESTDIR)$(PREFIX)/share/applications
ICONDIR  = $(DESTDIR)$(PREFIX)/share/icons/hicolor/scalable/apps

.PHONY: all build install uninstall clean run

all: build

build:
	cargo build --release
	cargo build --release --manifest-path backend/Cargo.toml

install: build
	install -Dm755 target/release/velo                         "$(BINDIR)/velo"
	install -Dm755 backend/target/release/velo-backend         "$(BINDIR)/velo-backend"
	install -Dm644 assets/velo.desktop                         "$(APPDIR)/velo.desktop"
	install -Dm644 assets/velo.svg                             "$(ICONDIR)/velo.svg"
	@update-desktop-database "$(APPDIR)" 2>/dev/null || true
	@gtk-update-icon-cache -f -t "$(DESTDIR)$(PREFIX)/share/icons/hicolor" 2>/dev/null || true
	@echo ""
	@echo "  Velo installed to $(PREFIX)"
	@echo "  Run: velo"

uninstall:
	rm -f "$(BINDIR)/velo" "$(BINDIR)/velo-backend"
	rm -f "$(APPDIR)/velo.desktop"
	rm -f "$(ICONDIR)/velo.svg"
	@update-desktop-database "$(APPDIR)" 2>/dev/null || true
	@echo "Velo uninstalled."

clean:
	cargo clean
	cargo clean --manifest-path backend/Cargo.toml

run:
	cargo run
