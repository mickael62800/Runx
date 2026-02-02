# Runx

CLI universel pour orchestrer des tâches avec watch intelligent, **live dashboard**, exécution parallèle et annotation IA des tests.

> Version 0.3.0 - Exécution parallèle, cache intelligent, détection flaky, TUI, notifications, monorepo et annotation IA

## Nouvelles fonctionnalités v0.3.0

- **Exécution parallèle** avec contrôle du nombre de workers
- **Cache intelligent** pour skip les tâches inchangées
- **Détection de tests flaky** avec retry automatique
- **Smart test selection** basé sur les changements git (`--affected`)
- **Profils** (dev, ci) pour configurations différentes
- **Notifications** Slack, Discord, GitHub
- **TUI interactive** pour contrôle visuel
- **Support monorepo** avec workspaces
- **Intégration coverage** (LCOV, Cobertura)
- **Annotation IA des tests** via Claude/GPT

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

## Démarrage rapide

1. Créer un fichier `runx.toml` à la racine de votre projet :

```toml
[project]
name = "mon-projet"
default_profile = "dev"

[tasks.build]
cmd = "cargo build"
watch = ["src/**/*.rs"]
parallel = true

[tasks.test]
cmd = "cargo test"
depends_on = ["build"]
category = "unit"
retry = 2
```

2. Lancer une tâche :

```bash
runx run build
```

3. Lancer en parallèle :

```bash
runx run --parallel --workers 4
```

## Commandes

| Commande | Description |
|----------|-------------|
| `runx run [task]` | Exécute une tâche (ou toutes) |
| `runx run --parallel` | Exécution parallèle |
| `runx run --affected` | Tests affectés par git uniquement |
| `runx watch [task]` | Surveille les fichiers et relance |
| `runx list` | Liste les tâches disponibles |
| `runx serve` | Lance le **live dashboard** temps réel |
| `runx tui` | Interface terminal interactive |
| `runx cache show` | Statistiques du cache |
| `runx cache clear` | Vider le cache |
| `runx profiles list` | Liste les profils |
| `runx annotate file <path>` | Annoter les tests avec IA |

### Options de `runx run`

```bash
runx run --parallel              # Exécution parallèle
runx run --workers 8             # Nombre de workers
runx run --affected              # Tests affectés uniquement
runx run --since main            # Depuis une branche/commit
runx run --no-cache              # Ignorer le cache
runx run --fail-fast             # Arrêter au premier échec
runx run --profile ci            # Utiliser un profil
runx run --filter "test-*"       # Filtrer par pattern
runx run --report                # Générer rapport HTML
runx run -v                      # Mode verbose
```

## Configuration complète

### Structure du fichier `runx.toml`

```toml
[project]
name = "nom-du-projet"
default_profile = "dev"

# Profils pour différents environnements
[profiles.dev]
parallel = false
cache = true
verbose = true

[profiles.ci]
parallel = true
workers = 4
cache = true
fail_fast = true
notifications = true

# Workspaces (monorepo)
[workspaces]
packages = ["packages/*", "apps/*"]

# Cache global
[cache]
enabled = true
ttl_hours = 24

# Notifications
[notifications]
enabled = true
on_failure = true

[notifications.slack]
webhook_url = "${SLACK_WEBHOOK_URL}"

[notifications.discord]
webhook_url = "${DISCORD_WEBHOOK_URL}"

[notifications.github]
enabled = true

# Configuration IA pour annotations
[ai]
provider = "anthropic"           # ou "openai"
api_key = "${ANTHROPIC_API_KEY}"
model = "claude-sonnet-4-20250514"
language = "fr"                  # en, fr, es, de

# Tâches
[tasks.build]
cmd = "cargo build"
cwd = "backend"
watch = ["src/**/*.rs"]
depends_on = []
parallel = true
workers = 2
category = "build"

[tasks.test]
cmd = "cargo test"
depends_on = ["build"]
category = "unit"
retry = 3                        # Retry sur échec
retry_delay_ms = 1000
timeout_seconds = 300
results = "test-results.xml"     # JUnit XML
coverage = true
coverage_format = "lcov"
coverage_path = "coverage/lcov.info"
coverage_threshold = 80
inputs = ["src/**/*.rs"]         # Pour cache
outputs = ["target/"]
```

## Exécution Parallèle

Runx calcule automatiquement les niveaux de dépendance et exécute les tâches indépendantes en parallèle :

```bash
runx run --parallel --workers 4
```

```
⚡ 3 execution levels, 4 workers

→ Level 0: build-a, build-b, lint (parallel)
→ Level 1: test-unit, test-integration (parallel)
→ Level 2: e2e (sequential - depends on previous)

✓ All 6 task(s) completed successfully (4523ms)
```

## Cache Intelligent

Le cache permet de skip les tâches dont les inputs n'ont pas changé :

```bash
# Afficher les statistiques
runx cache show

# Cache Statistics:
#   Total entries:  12
#   Valid entries:  10
#   Expired:        2
#   Time saved:     45230ms

# Vider le cache
runx cache clear
```

## Détection Flaky + Retry

Runx détecte automatiquement les tests flaky et peut les quarantiner :

```toml
[tasks.test]
cmd = "cargo test"
retry = 3                # Retry jusqu'à 3 fois
retry_delay_ms = 1000    # Délai entre retries
```

```
↻ Retrying attempt 2 of 3 (waiting 1000ms)
⚠ test-unit is flaky (passed on attempt 2/3)
```

## Smart Test Selection

Exécuter uniquement les tests affectés par les changements git :

```bash
# Tests affectés depuis HEAD
runx run --affected

# Tests affectés depuis une branche
runx run --affected --since main

# Avec base explicite
runx run --affected --base develop
```

## Profils

Définir des configurations différentes selon l'environnement :

```toml
[profiles.dev]
parallel = false
cache = true
verbose = true

[profiles.ci]
parallel = true
workers = 4
fail_fast = true
notifications = true
```

```bash
runx run --profile ci
runx profiles list
```

## TUI Interactive

Interface terminal pour contrôle visuel :

```bash
runx tui
```

```
┌──────────────────────────────────────────────────────────────────┐
│ Runx TUI - my-project                                   [Ctrl+C] │
├────────────────────────┬─────────────────────────────────────────┤
│ Tasks                  │ Output: test-unit                       │
│ ───────────────────    │                                         │
│ > build      ✓ 1.2s    │ running 45 tests                        │
│   test-unit  ● running │ test src/config.rs ... ok               │
│   test-e2e   ○ pending │ test src/runner.rs ... ok               │
│   lint       ○ pending │ test src/graph.rs ... FAILED            │
├────────────────────────┴─────────────────────────────────────────┤
│ [r]etry [s]kip [Enter]view [/]search [q]uit         2/4 complete │
└──────────────────────────────────────────────────────────────────┘
```

## Annotation IA des Tests

Générer automatiquement des descriptions pour vos tests avec Claude ou GPT :

```bash
# Annoter un fichier
runx annotate file src/tests/auth_test.rs --language fr

# Annoter tout le projet
runx annotate all --pattern "**/*test*.rs"

# Afficher les annotations
runx annotate show
runx annotate show --test-type unit
runx annotate show --tag "auth"

# Exporter en JSON
runx annotate export -o annotations.json
```

Exemple de sortie :

```
🤖 Annotating tests in src/tests/auth_test.rs...
✓ Annotated 3 test(s)

  ▸ test_login_success
    Vérifie que l'authentification réussit avec des identifiants valides
    Purpose: S'assurer que le flux d'authentification fonctionne
    Tests: login()
    Type: integration
    Tags: auth, login, security
```

Configuration :

```toml
[ai]
provider = "anthropic"           # ou "openai"
api_key = "${ANTHROPIC_API_KEY}" # Variable d'environnement
language = "fr"                  # en, fr, es, de
```

## Notifications

Recevoir des notifications sur Slack, Discord ou GitHub :

```toml
[notifications]
enabled = true
on_failure = true    # Notifier seulement sur échec

[notifications.slack]
webhook_url = "${SLACK_WEBHOOK_URL}"

[notifications.discord]
webhook_url = "${DISCORD_WEBHOOK_URL}"

[notifications.github]
enabled = true
```

## Support Monorepo

Gérer plusieurs packages dans un monorepo :

```toml
[workspaces]
packages = ["packages/*", "apps/*"]
```

```bash
runx run --workspace           # Tous les packages
runx run --package api         # Package spécifique
```

Les tâches sont préfixées : `api:build`, `web:test`, etc.

## Live Dashboard

Lancer le dashboard temps réel :

```bash
runx serve              # Port 3000 par défaut
runx serve --port 8080  # Port personnalisé
```

Ouvrez http://localhost:3000 dans votre navigateur.

### Fonctionnalités

- **Mise à jour temps réel** via WebSocket
- **Base de données SQLite** pour l'historique des runs
- **Statistiques** : total runs, taux de réussite, durée moyenne
- **Graphiques ECharts** : tendances sur 7 jours, résultats récents
- **Historique** : sidebar cliquable avec détails de chaque run
- **Filtrage** par catégorie (unit, integration, e2e, lint...)
- **Annotations IA** affichées pour chaque test

## Tests E2E avec serveur

Runx supporte les processus en arrière-plan pour les tests E2E :

```toml
[tasks.dev-server]
cmd = "npm run dev"
background = true
ready_when = "Local:"        # Attend ce texte dans stdout
ready_timeout = 60           # Timeout max en secondes

[tasks.e2e]
cmd = "npx playwright test"
depends_on = ["dev-server"]  # Attend que le serveur soit prêt
category = "e2e"
```

## Mode Watch

Surveiller les fichiers et relancer automatiquement :

```bash
runx watch           # Surveille toutes les tâches
runx watch build     # Surveille uniquement "build"
```

Exclusions automatiques : `target/`, `node_modules/`, `dist/`, `out/`, `.git/`

## Exemple complet

```toml
[project]
name = "fullstack-app"
default_profile = "dev"

[profiles.dev]
parallel = false
cache = true
verbose = true

[profiles.ci]
parallel = true
workers = 4
fail_fast = true
notifications = true

[cache]
enabled = true
ttl_hours = 24

[notifications.slack]
webhook_url = "${SLACK_WEBHOOK_URL}"

[ai]
provider = "anthropic"
api_key = "${ANTHROPIC_API_KEY}"
language = "fr"

# Build
[tasks.build-backend]
cmd = "cargo build --release"
cwd = "backend"
watch = ["backend/src/**/*.rs"]
category = "build"
parallel = true

[tasks.build-frontend]
cmd = "npm run build"
cwd = "frontend"
watch = ["frontend/src/**/*.vue"]
category = "build"
parallel = true

# Tests unitaires
[tasks.test-backend]
cmd = "cargo test"
cwd = "backend"
depends_on = ["build-backend"]
category = "unit"
retry = 2
coverage = true
coverage_threshold = 80

[tasks.test-frontend]
cmd = "npm test"
cwd = "frontend"
depends_on = ["build-frontend"]
category = "unit"
retry = 2

# Serveur de dev (arrière-plan)
[tasks.dev-server]
cmd = "npm run dev"
cwd = "frontend"
background = true
ready_when = "ready in"
ready_timeout = 30

# Tests E2E
[tasks.e2e]
cmd = "npx playwright test"
cwd = "frontend"
depends_on = ["build-backend", "dev-server"]
category = "e2e"
timeout_seconds = 300

# Lint
[tasks.lint]
cmd = "cargo clippy && npm run lint"
category = "lint"
parallel = true
```

## Licence

MIT
