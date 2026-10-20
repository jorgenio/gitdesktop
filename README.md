# Git-Desktop Jorgenio

Cliente Git de escritorio **100% en Rust** con la filosofía de GitHub Desktop:
una interfaz simple de 3 paneles para gestionar cambios, commits, ramas,
pull, push y merge sin tocar la terminal.

- **Objetivo:** Ubuntu Linux (primario) + Windows
- **UI:** [iced](https://iced.rs) 0.14 · **Motor Git:** libgit2 (`git2`) + CLI `git`
- **Plan completo:** [`PLAN.md`](./PLAN.md)

## Estado (v0.1.1)

- [x] Gestor Git completo: abrir/clone, stage/unstage, discard, commits,
      diff unificado, historial con detalle
- [x] Ramas: crear/cambiar/eliminar + merge con resolución de conflictos
- [x] Fetch/Pull/Push/Publish con progreso y credenciales del sistema
- [x] Barra de título personalizada con menús (estilo GitHub Desktop)
- [x] Instaladores: `.deb` + AppImage (Linux) y MSI (Windows) en cada tag

## Instalación

### Ubuntu / Debian (`.deb`)

```sh
sudo dpkg -i git-desktop-jorgenio-linux-x86_64.deb
# (si faltan dependencias: sudo apt -f install)
```

### Ubuntu / cualquier Linux (AppImage)

```sh
chmod +x git-desktop-jorgenio-linux-x86_64.AppImage
./git-desktop-jorgenio-linux-x86_64.AppImage
```

### Windows (MSI)

Descarga `git-desktop-jorgenio-windows-x86_64.msi` y ejecuta el instalador
(crea accesos directos en el menú Inicio; el binario suelto `.exe` también
funciona sin instalación).

Los artefactos están en la [página de releases](https://github.com/jorgenio/gitdesktop/releases).

## Desarrollo

```sh
cargo run -p gitdesktop-app
```

Requiere Rust estable (≥ 1.88): [rustup.rs](https://rustup.rs).

La configuración se guarda en `~/.config/git-desktop-jorgenio/gitdesktop.toml`
(Linux) o `%APPDATA%\git-desktop-jorgenio\gitdesktop.toml` (Windows).

El ejecutable se llama `git-desktop-jorgenio`.

## Licencia

MIT
