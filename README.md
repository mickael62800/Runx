# Runx

**Rust Test Explorer** - CLI pour découvrir, exécuter et gérer vos tests Rust avec une interface TUI interactive.

> Version 1.0.0

## Fonctionnalités

- **Découverte automatique** des tests via `cargo test -- --list`
- **TUI interactive** avec vue arborescente des tests
- **Exécution en temps réel** avec sortie streaming
- **Mode watch** avec détection des tests affectés
- **Filtrage** par nom et statut (passed/failed/pending)

## Installation

### Prérequis

- [Rust](https://rustup.rs/) (1.70+)

### Depuis les sources

```bash
git clone https://github.com/mickael62800/Runx.git
cd Runx
cargo install --path .
```

### Vérifier l'installation

```bash
runx --version
```

## Utilisation

### TUI Interactive (par défaut)

```bash
runx
```

Lance l'interface terminal interactive avec :
- Vue arborescente des modules et tests
- Navigation clavier
- Exécution des tests sélectionnés
- Filtrage en temps réel

### Commandes CLI

| Commande | Description |
|----------|-------------|
| `runx` | Lance la TUI (par défaut) |
| `runx run` | Exécute tous les tests |
| `runx run "pattern"` | Exécute les tests correspondant au pattern |
| `runx list` | Liste tous les tests découverts |
| `runx list "pattern"` | Liste les tests filtrés |
| `runx watch` | Mode watch - relance les tests affectés |
| `runx discover` | Découvre et affiche les statistiques |

### Options

```bash
runx run -v              # Mode verbose
runx list --full         # Affiche les chemins complets
```

## Raccourcis TUI

| Touche | Action |
|--------|--------|
| `j/k` ou `↑/↓` | Naviguer haut/bas |
| `Enter` | Exécuter le test/module sélectionné |
| `Space` | Déplier/replier un module |
| `a` | Exécuter tous les tests |
| `f` | Exécuter les tests échoués |
| `d` | Re-découvrir les tests |
| `/` | Mode filtre (saisie) |
| `1` | Afficher tous les tests |
| `2` | Afficher uniquement les passed |
| `3` | Afficher uniquement les failed |
| `4` | Afficher uniquement les pending |
| `Tab` | Changer le mode de filtre |
| `e` | Tout déplier |
| `c` | Tout replier |
| `q` ou `Esc` | Quitter |

## Exemple

```
$ runx

┌─ Runx - Rust Test Explorer ─────────────────────────────────────┐
│                                                                  │
│  ▼ src                                                          │
│    ▼ config                                                     │
│      ✓ test_load_config                                         │
│      ✓ test_default_values                                      │
│    ▼ runner                                                     │
│      ✓ test_run_single                                          │
│      ✗ test_run_parallel                                        │
│      ○ test_timeout                                             │
│                                                                  │
│  Filter: [                    ] Mode: All                       │
│                                                                  │
│  Tests: 5 total | 3 passed | 1 failed | 1 pending              │
└──────────────────────────────────────────────────────────────────┘
```

## Mode Watch

Le mode watch surveille les fichiers sources et relance automatiquement les tests affectés :

```bash
runx watch
runx watch "auth"  # Surveille uniquement les tests contenant "auth"
```

Fichiers exclus automatiquement : `target/`, `.git/`

## Architecture

```
src/
├── main.rs              # Point d'entrée CLI (clap)
├── test_model.rs        # Structures Test, TestNode, TestStatus
├── discovery.rs         # Découverte via cargo test --list
├── test_runner.rs       # Exécution avec sortie streaming
├── affected.rs          # Mapping fichier → tests
├── watcher.rs           # Surveillance fichiers
└── tui/                 # Interface terminal (ratatui)
    ├── app.rs           # État de l'application
    ├── ui.rs            # Rendu
    ├── events.rs        # Gestion clavier
    └── widgets/
        └── test_tree.rs # Widget arbre de tests
```

## Licence

MIT
