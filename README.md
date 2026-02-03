# Runx

**Rust Test Explorer** - CLI pour découvrir, exécuter et gérer vos tests Rust avec une interface TUI interactive et un dashboard web temps réel.

> Version 2.0.0

## Fonctionnalités

- **Découverte automatique** des tests via `cargo test -- --list`
- **TUI interactive** avec vue arborescente des tests
- **Dashboard web** avec graphiques et visualisations en temps réel
- **Système d'artifacts** pour afficher des graphiques personnalisés depuis vos tests
- **Mode watch** avec détection des tests affectés et mise à jour WebSocket
- **Debug panel** pour monitorer Vue.js/Pinia et Tauri en temps réel
- **Filtrage** par nom et statut (passed/failed/pending)
- **Historique** des exécutions avec statistiques

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

### Dashboard Web

```bash
runx dashboard              # Lance le dashboard sur le port 3000
runx dashboard --port 8080  # Port personnalisé
runx dashboard --watch      # Mode watch avec mise à jour temps réel
```

Le dashboard offre :
- **Onglet Tests** : Visualisation des résultats avec graphiques
- **Onglet Debug** : Monitoring temps réel Vue/Pinia et Tauri
- Historique des exécutions
- Barre de recherche pour filtrer les tests

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
| `runx dashboard` | Lance le dashboard web |
| `runx dashboard --watch` | Dashboard avec mode watch |

### Options

```bash
runx run -v                  # Mode verbose
runx list --full             # Affiche les chemins complets
runx dashboard --port 8080   # Port personnalisé
runx dashboard --watch       # Active le mode watch
```

## runx-charts : Templates de Graphiques

La bibliothèque `runx-charts` fournit des templates prêts à l'emploi pour créer des graphiques facilement.

### Installation

```toml
# Cargo.toml
[dependencies]
runx-charts = { path = "path/to/runx-charts" }
```

### Templates disponibles

#### 1. Performance (Latence, Throughput)

```rust
use runx_charts::prelude::*;

#[test]
fn test_api_latency() {
    Performance::latency("test_api_latency")
        .title("API Endpoint Latency")
        .labels(&["GET /users", "POST /orders", "GET /products"])
        .data(&[12.5, 45.2, 8.1])
        .threshold(50.0)  // SLA en ms
        .save();
}

#[test]
fn test_latency_percentiles() {
    Performance::latency_percentiles("test_percentiles")
        .title("Latency Distribution")
        .p50(12.0)
        .p90(25.0)
        .p99(45.0)
        .p999(120.0)
        .save();
}

#[test]
fn test_throughput() {
    Performance::throughput("test_throughput")
        .title("Server Throughput")
        .data(&[1000.0, 1200.0, 1400.0])
        .compare(&[900.0, 1000.0, 1100.0])  // Baseline
        .save();
}
```

#### 2. Memory Profiling

```rust
#[test]
fn test_memory_usage() {
    Memory::usage("test_memory")
        .title("Application Memory")
        .samples(&[100.0, 110.0, 120.0, 115.0])
        .timestamps(&["0s", "10s", "20s", "30s"])
        .limit(200.0)  // Limite en MB
        .save();
}

#[test]
fn test_memory_breakdown() {
    Memory::breakdown("test_allocation")
        .title("Memory Breakdown")
        .heap(85.0)
        .stack(12.0)
        .static_mem(8.0)
        .save();
}
```

#### 3. API Response Times

```rust
#[test]
fn test_api_endpoints() {
    Api::response_times("test_api")
        .title("API Response Times")
        .endpoint("GET /users", 45.0)
        .endpoint("POST /orders", 120.0)
        .endpoint("DELETE /session", 25.0)
        .sla(100.0)
        .save();
}

#[test]
fn test_status_codes() {
    Api::status_codes("test_errors")
        .title("HTTP Status Distribution")
        .ok(950)
        .client_error(30)
        .server_error(5)
        .save();
}
```

#### 4. Test Coverage

```rust
#[test]
fn test_coverage_by_module() {
    Coverage::by_module("test_coverage")
        .title("Coverage by Module")
        .module("src/api", 85.0)
        .module("src/db", 72.0)
        .module("src/utils", 95.0)
        .target(80.0)
        .save();
}

#[test]
fn test_coverage_total() {
    Coverage::total("test_total_cov")
        .title("Overall Coverage")
        .percentage(82.5)
        .lines(1650, 2000)
        .target(80.0)
        .save();
}

#[test]
fn test_coverage_trend() {
    Coverage::trend("test_trend")
        .title("Coverage Over Time")
        .point("Jan", 65.0)
        .point("Feb", 70.0)
        .point("Mar", 78.0)
        .target(80.0)
        .save();
}
```

### Types de graphiques supportés

| Type | Description | Templates |
|------|-------------|-----------|
| `line` | Graphique linéaire | Performance, Coverage trend |
| `bar` | Graphique en barres | Throughput, API, Coverage |
| `area` | Graphique en aires | Memory usage |
| `pie` | Graphique circulaire | Memory breakdown, Status codes |
| `gauge` | Jauge (valeur unique) | Coverage total |

### Catégories dans le Dashboard

Le dashboard organise automatiquement les graphiques par catégorie :
- ⚡ **Performance** : Latence, throughput, percentiles
- 🧠 **Memory** : Usage mémoire, allocations
- 🌐 **API** : Temps de réponse, status codes
- 📈 **Coverage** : Couverture par module, tendances

## Debug Panel (Vue/Pinia & Tauri)

Le dashboard inclut un onglet Debug pour monitorer en temps réel les événements de vos applications Vue.js et Tauri.

### Envoyer des événements depuis votre app

```javascript
// Fonction utilitaire
async function sendDebugEvent(source, eventType, name, payload, error = null) {
  await fetch('http://localhost:3000/api/debug', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      source,       // "pinia", "tauri", "vue", "custom"
      event_type,   // "mutation", "action", "command", "response", "error"
      name,
      payload,
      timestamp: new Date().toISOString(),
      error
    })
  });
}

// Exemples d'utilisation
sendDebugEvent('pinia', 'mutation', 'user/setBalance', { balance: 1500 });
sendDebugEvent('pinia', 'action', 'user/login', { email: 'test@example.com' });
sendDebugEvent('tauri', 'command', 'get_portfolio', { user_id: 1 });
sendDebugEvent('tauri', 'response', 'get_portfolio', { balance: 10000 });
sendDebugEvent('tauri', 'error', 'place_order', null, 'Insufficient funds');
```

### Intégration Pinia (plugin)

```javascript
// stores/index.js
import { createPinia } from 'pinia'

const pinia = createPinia()

pinia.use(({ store }) => {
  store.$onAction(({ name, args, after, onError }) => {
    sendDebugEvent('pinia', 'action', `${store.$id}/${name}`, args[0])

    after((result) => {
      sendDebugEvent('pinia', 'action_result', `${store.$id}/${name}`, result)
    })

    onError((error) => {
      sendDebugEvent('pinia', 'error', `${store.$id}/${name}`, null, error.message)
    })
  })
})
```

### Intégration Tauri (wrapper)

```javascript
// utils/tauri.js
import { invoke as tauriInvoke } from '@tauri-apps/api/tauri'

export async function invoke(cmd, args) {
  sendDebugEvent('tauri', 'command', cmd, args)

  try {
    const result = await tauriInvoke(cmd, args)
    sendDebugEvent('tauri', 'response', cmd, result)
    return result
  } catch (error) {
    sendDebugEvent('tauri', 'error', cmd, null, error.toString())
    throw error
  }
}
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

## Exemple TUI

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

Avec le dashboard :
```bash
runx dashboard --watch
```
Les résultats sont mis à jour en temps réel via WebSocket.

Fichiers exclus automatiquement : `target/`, `node_modules/`, `dist/`, `.git/`

## API REST

Le dashboard expose une API REST :

| Endpoint | Méthode | Description |
|----------|---------|-------------|
| `/api/stats` | GET | Statistiques globales |
| `/api/runs` | GET | Liste des exécutions |
| `/api/runs/:id` | GET | Détails d'une exécution |
| `/api/artifacts` | GET | Liste des artifacts |
| `/api/artifacts/:test_name` | GET | Artifact d'un test |
| `/api/debug` | POST | Envoyer un événement debug |
| `/api/clear-history` | POST | Effacer l'historique |
| `/api/shutdown` | POST | Arrêter le serveur |
| `/ws` | WebSocket | Mises à jour temps réel |

## Architecture

```
src/
├── main.rs              # Point d'entrée CLI (clap)
├── lib.rs               # Exports de la bibliothèque
├── test_model.rs        # Structures Test, TestNode, TestStatus
├── discovery.rs         # Découverte via cargo test --list
├── test_runner.rs       # Exécution avec sortie streaming
├── affected.rs          # Mapping fichier → tests
├── watcher.rs           # Surveillance fichiers
├── server.rs            # Serveur HTTP/WebSocket (Axum)
├── artifacts.rs         # Gestion des artifacts de visualisation
├── db.rs                # Base de données SQLite
├── dashboard.html       # Interface web du dashboard
└── tui/                 # Interface terminal (ratatui)
    ├── app.rs           # État de l'application
    ├── ui.rs            # Rendu
    ├── events.rs        # Gestion clavier
    └── widgets/
        └── test_tree.rs # Widget arbre de tests
```

## Licence

MIT
