# Runx

CLI universel pour orchestrer des tâches avec watch intelligent et **live dashboard**.

> Version 0.2.0 - Maintenant avec dashboard temps réel et base de données SQLite

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

[tasks.build]
cmd = "cargo build"
watch = ["src/**/*.rs"]

[tasks.test]
cmd = "cargo test"
depends_on = ["build"]
category = "unit"
```

2. Lancer une tâche :

```bash
runx run build
```

3. Lancer toutes les tâches :

```bash
runx run
```

## Commandes

| Commande | Description |
|----------|-------------|
| `runx run [task]` | Exécute une tâche (ou toutes) |
| `runx run --report` | Exécute et génère un dashboard HTML statique |
| `runx watch [task]` | Surveille les fichiers et relance |
| `runx list` | Liste les tâches disponibles |
| `runx serve` | Lance le **live dashboard** temps réel |

### Options

```bash
runx run --report                      # Génère runx-report.html
runx run --report --report-path=out/   # Chemin personnalisé
runx -c autre.toml run build           # Fichier config alternatif
```

## Configuration

### Structure du fichier `runx.toml`

```toml
[project]
name = "nom-du-projet"

[tasks.nom-tache]
cmd = "commande à exécuter"
cwd = "sous-dossier"              # Optionnel: répertoire de travail
watch = ["src/**/*.rs"]           # Optionnel: patterns glob à surveiller
depends_on = ["autre-tache"]      # Optionnel: dépendances
category = "unit"                 # Optionnel: catégorie (unit, e2e, integration...)
results = "test-results.xml"      # Optionnel: chemin JUnit XML pour parsing détaillé
background = false                # Optionnel: tâche en arrière-plan
ready_when = "Server running"     # Optionnel: texte indiquant que le service est prêt
ready_timeout = 30                # Optionnel: timeout en secondes (défaut: 30)
```

### Catégories de tests

Les catégories permettent de filtrer dans le dashboard :

```toml
[tasks.test-unit]
cmd = "cargo test --lib"
category = "unit"

[tasks.test-integration]
cmd = "npm run test:integration"
category = "integration"

[tasks.test-e2e]
cmd = "npx playwright test"
category = "e2e"

[tasks.lint]
cmd = "cargo clippy"
category = "lint"
```

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

Runx :
1. Lance le serveur en arrière-plan
2. Attend que "Local:" apparaisse dans la sortie
3. Exécute les tests E2E
4. Arrête le serveur automatiquement

## Live Dashboard (v0.2.0)

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

### Parsing JUnit XML

Runx peut parser les rapports JUnit XML pour des détails précis :

```toml
[tasks.test]
cmd = "cargo test -- --format=junit > results.xml"
results = "results.xml"   # Chemin vers le fichier JUnit XML
category = "unit"
```

## Dashboard HTML (statique)

Générer un rapport HTML statique (sans serveur) :

```bash
runx run --report
```

Le dashboard statique inclut :
- Résumé global (passed/failed/durée)
- Graphique timeline des durées
- Graphique pie chart pass/fail
- Filtres par statut (All/Passed/Failed)
- Filtres par catégorie (unit/e2e/integration...)
- Recherche par nom de tâche
- Détail de chaque tâche

## Mode Watch

Surveiller les fichiers et relancer automatiquement :

```bash
runx watch           # Surveille toutes les tâches
runx watch build     # Surveille uniquement "build"
```

Exclusions automatiques :
- `target/`
- `node_modules/`
- `dist/`
- `out/`
- `.git/`

## Exemple complet

```toml
[project]
name = "fullstack-app"

# Build
[tasks.build-backend]
cmd = "cargo build --release"
cwd = "backend"
watch = ["backend/src/**/*.rs"]
category = "build"

[tasks.build-frontend]
cmd = "npm run build"
cwd = "frontend"
watch = ["frontend/src/**/*.vue", "frontend/src/**/*.ts"]
category = "build"

# Tests unitaires
[tasks.test-backend]
cmd = "cargo test"
cwd = "backend"
depends_on = ["build-backend"]
category = "unit"

[tasks.test-frontend]
cmd = "npm test"
cwd = "frontend"
depends_on = ["build-frontend"]
category = "unit"

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

# Lint
[tasks.lint]
cmd = "cargo clippy && npm run lint"
category = "lint"
```

## Licence

MIT
