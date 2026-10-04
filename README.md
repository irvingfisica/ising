# Ising

Implementación experimental de un modelo de Ising en Rust.

El proyecto está diseñado alrededor de una separación entre el **modelo genérico del sistema** y las implementaciones concretas utilizadas para realizar experimentos. El núcleo no supone una geometría particular: un sistema puede definirse sobre una topología arbitraria.

Además del núcleo, la librería incluye:

- un módulo para **ejecutar experimentos** de forma declarativa y exportar sus resultados (`simulaciones`);
- un **visor web** interactivo de esos resultados, con exportación a PDF (`visor`);
- **gráficas estáticas** en SVG de los mismos resultados (`graficas`, opcional).

## Inicio rápido

```bash
cargo run --release --example experimento
```

Esto simula una red de 32×32 en la temperatura crítica, imprime un resumen en consola y escribe en `datos/` los CSV con los resultados más un visor web. Para ver los resultados, abre `datos/visor.html` con doble clic.

Para generar además las gráficas estáticas en `datos/graficas/`:

```bash
cargo run --release --example graficas --features graficas -- datos
```

## Flujo de trabajo

```text
  Sistema ──► Experimento::simular ──► Resultado ──► escribir("datos")
                                                         │
                         ┌───────────────────────────────┼──────────────────────────┐
                         ▼                               ▼                          ▼
              visor::instalar("datos")      graficas::graficar(...)       salida::Salida::leer
              visor.html + datos.js         SVG estáticos                 (análisis propio en Rust)
              (interactivo, PDF)
```

### 1. Construir el sistema

```rust
use rand::SeedableRng;
use rand::rngs::StdRng;
use ising::sistema::{Sistema, Inicial, Dinamica};

let mut rng = StdRng::seed_from_u64(123456789);

// Red cuadrada L×L con condiciones periódicas: (L, J, H, T, estado inicial, rng)
let mut sistema = Sistema::square_grid(128, 1.0, 0.0, 2.269185314, Inicial::Random, &mut rng);
```

También puede construirse sobre una red arbitraria con `Sistema::from_edges` (ver [Topología](#topología)).

### 2. Definir y ejecutar el experimento

Un `Experimento` es un burn-in seguido de un número de sweeps de medición. Durante la medición, cada `PlanMedicion` mide sus métricas cada `cada` sweeps. Lo habitual es un plan barato en cada sweep y otro con las métricas espaciales, que son costosas, más espaciado:

```rust
use ising::simulaciones::{Experimento, PlanMedicion, Metrica};

let experimento = Experimento::new(50_000, 50_000, Dinamica::Glauber) // burn-in, mediciones, dinámica
    .medir(PlanMedicion::cada(1, vec![
        Metrica::Magnetizacion,
        Metrica::MagnetizacionAbsoluta,
        Metrica::Energia,
        Metrica::C1,
    ]))
    .medir(PlanMedicion::cada(100, vec![
        Metrica::Correlaciones { r_max: 64 },
        Metrica::ClustersGeometricos,
        Metrica::ClustersFK,
    ]))
    .progreso(1000); // opcional: imprime el avance cada 1000 sweeps

let resultado = experimento.simular(&mut sistema, &mut rng)?;
```

`simular` recibe el sistema por referencia mutable, así que al terminar se puede seguir usando (por ejemplo, para encadenar experimentos a distintas temperaturas). Antes de correr, valida que no haya dos planes con la misma cadencia, métricas repetidas en un plan ni `r_max = 0`.

Métricas disponibles:

| Métrica | Qué mide | Columna / archivo |
|---|---|---|
| `Magnetizacion` | m = (1/N) Σ sᵢ | `M` en `serie.csv` |
| `MagnetizacionAbsoluta` | \|m\| | `abs_M` |
| `Energia` | energía por espín, e = −(J/2N) Σᵢ Σ_{v(i)} sᵢ s_v − (H/N) Σ sᵢ | `E` |
| `C1` | correlación a primeros vecinos ⟨sᵢ s_v⟩ | `C1` |
| `Correlaciones { r_max }` | C(r) para r = 1..r_max, con r la distancia en la red (capas de vecinos) | `correlaciones.csv` |
| `ClustersGeometricos` | tamaños de los clusters de espines iguales | `clusters_geometricos.csv` |
| `ClustersFK` | tamaños de los clusters de Fortuin–Kasteleyn (p = 1 − e^(−2J/T)) | `clusters_fk.csv` |

### 3. Revisar y exportar el resultado

```rust
println!("{}", resultado);     // resumen en consola: estadísticas, C(r) y clusters
resultado.escribir("datos")?;  // CSV en el directorio indicado (se crea si no existe)
```

`Resultado` también da acceso directo a los datos: `resultado.planes` (series crudas), `resultado.serie(&Metrica::Energia)`, `resultado.estadisticas()` y `Resultado::correlaciones_medias(...)`.

Archivos que escribe `escribir`:

| Archivo | Contenido |
|---|---|
| `parametros.csv` | N, J, H, T, dinámica, burn-in y mediciones |
| `resumen.csv` | una fila por observable: cadencia, n, media, ⟨x²⟩, varianza, τ_int y error |
| `serie.csv` | `sweep` y una columna por cada métrica escalar |
| `correlaciones.csv` | r, pares por snapshot, C(r), Cc(r) = C(r) − ⟨m⟩² y Cc_por_snapshot = ⟨C(r) − m²⟩ |
| `clusters_geometricos.csv`, `clusters_fk.csv` | snapshot, sweep, índice y tamaño de cada cluster, de mayor a menor |

Notas sobre la estadística:

- τ_int es el tiempo de autocorrelación integrado, 1/2 + Σ ρ(t), cortando la suma en el primer ρ(t) ≤ 0. En `resumen.csv` está en unidades de **mediciones**; para pasarlo a sweeps se multiplica por la cadencia.
- El error es el error estándar corregido por autocorrelación: √(2 τ_int · var / n).
- Del número de clusters y del tamaño del mayor cluster en cada snapshot también se calculan estadísticas, como si fueran una serie más.
- Si el mismo tipo de archivo lo producen varios planes, se agrega el sufijo `_cada{k}` (por ejemplo `serie_cada1.csv` y `serie_cada10.csv`).

### 4. Visor web

```rust
use ising::visor;

let ruta = visor::instalar("datos")?; // escribe datos/visor.html y datos/datos.js
```

`instalar` copia `visor.html` en el directorio y empaqueta todos los CSV en `datos.js`, de modo que **el visor se abre con doble clic, sin servidor**. Para una carpeta de resultados ya existente (por ejemplo, de una corrida anterior):

```bash
cargo run --example instalar_visor -- ruta/a/datos
```

El visor también puede usarse sin `datos.js`: abre `visor.html` y arrastra la carpeta o los CSV, o usa los botones *Abrir carpeta…* / *Abrir archivos…*.

Requisitos: un navegador moderno y conexión a internet (D3 se carga desde cdnjs).

Qué muestra:

- **Encabezado:** parámetros del experimento.
- **Ventana de análisis:** arrastrando sobre la serie se elige un rango de sweeps y **todas las estadísticas y gráficas se recalculan** en ese rango (por ejemplo, para descartar más burn-in sin volver a simular). Doble clic o *Restablecer* vuelve a la serie completa; también se puede escribir el rango a mano.
- **Estadísticas:** media ± error, varianza, τ_int (en sweeps) y número de mediciones efectivas n/2τ_int de cada observable. Además, el cumulante de Binder U₄ = 1 − ⟨m⁴⟩/3⟨m²⟩², la susceptibilidad χ = N(⟨m²⟩ − ⟨|m|⟩²)/T y el calor específico C_v = N·var(e)/T².
- **Series:** cada métrica con su media; el cursor está sincronizado entre las gráficas.
- **Histograma** y **autocorrelación ρ(t)** de la métrica elegida, con τ_int marcado.
- **Correlación espacial:** C(r) y Cc(r), en escala lineal o log-log, con la referencia r^(−1/4) (η = 1/4 en T_c).
- **Distribución de tamaños de cluster n(s):** bins logarítmicos, con las pendientes de Fisher en T_c (τ = 31/15 para FK, 379/187 para geométricos). Opcionalmente se excluye el mayor cluster de cada snapshot.
- **Clusters por snapshot:** fracción del mayor cluster y número de clusters.

Los controles (métrica, bins, escala, referencias) están en la fila sobre el histograma. *Tema* alterna entre modo claro y oscuro.

**Generar PDF:** el botón pasa a una maqueta tamaño carta de dos páginas (termodinámica y estructura espacial) y abre el diálogo de impresión; ahí se elige **"Guardar como PDF"** como destino. El PDF refleja lo que está seleccionado en ese momento (ventana, métrica, escala, referencias). `Ctrl+P` produce el mismo resultado.

Limitaciones:

- `correlaciones.csv` solo guarda el promedio sobre snapshots, así que C(r) no se recalcula con la ventana de análisis.
- Con archivos de clusters sin columna `sweep` (formato anterior), la ventana no los filtra y su τ_int se reporta en snapshots.
- `datos.js` duplica el contenido de los CSV; con corridas grandes puede pesar decenas de MB.

El código del visor está en `visor/visor.html` y se incrusta en la librería con `include_str!`, así que cualquier cambio ahí queda en el binario al recompilar.

### 5. Gráficas estáticas (opcional)

Para figuras fijas (artículos, presentaciones), el módulo `graficas` usa `plotters` y está detrás del feature `graficas`, de modo que el núcleo no carga esa dependencia:

```bash
cargo run --release --example graficas --features graficas -- ruta/a/datos [ruta/destino]
```

O desde código:

```rust
let rutas = ising::graficas::graficar("datos", "datos/graficas")?;
```

Escribe un SVG por gráfica: `serie_{métrica}.svg`, `histograma_M.svg`, `autocorrelacion_M.svg`, `correlaciones.svg`, `clusters_distribucion.svg`, `clusters_mayor.svg` y `clusters_numero.svg`. El destino por defecto es `ruta/a/datos/graficas`.

### 6. Leer resultados desde Rust

`salida::Salida::leer("datos")` carga los CSV de un directorio de salida (parámetros, serie, correlaciones y clusters) para análisis propios. No requiere ningún feature.

### Usar la librería desde otro proyecto

```toml
[dependencies]
ising = { path = "../ising" }                            # núcleo, experimentos y visor
# ising = { path = "../ising", features = ["graficas"] } # + gráficas SVG
```

El proyecto `atlas` es un ejemplo completo: define las constantes del experimento, lo ejecuta, escribe los CSV e instala el visor.

## Estructura del proyecto

```text
src/
├── lib.rs
├── sistema.rs        núcleo: Sistema, Celda, Estado, Dinamica, Inicial, UnionFind
├── simulaciones.rs   Experimento, PlanMedicion, Metrica, Resultado, estadística de series
├── salida.rs         lectura de los CSV de salida
├── visor.rs          instalación del visor web
└── graficas.rs       gráficas SVG (feature "graficas")
visor/
└── visor.html        visor web (D3)
examples/
├── experimento.rs    experimento completo con el módulo simulaciones + visor
├── instalar_visor.rs instala el visor en una carpeta de resultados existente
├── graficas.rs       genera las gráficas SVG
├── metricas.rs       métricas calculadas a mano con la API del núcleo
├── simulacion.rs     simulación de una instancia en grid cuadrado
├── ensamble.rs       ensambles de réplicas en un barrido de temperaturas
├── ensamble_meta.rs  ensambles con distintas proporciones iniciales
└── common/
    └── simulacion_sqgrids.rs  código compartido de los experimentos en grids
```

## Arquitectura

```text
                         ising
                           │
        ┌──────────────────┼───────────────────────┐
        │                  │                       │
      core           experimentos            visualización
        │                  │                       │
   sistema.rs       simulaciones.rs         visor.rs (web, PDF)
                           │                graficas.rs (SVG)
                    salida.rs (lectura)
```

### Core

El núcleo contiene la representación y dinámica de un sistema de Ising sin imponer una geometría específica.

Un `Sistema` está compuesto por elementos (`Celda`) y una topología que determina las relaciones entre ellos. La topología puede representarse mediante identificadores y posteriormente compilarse a índices enteros para realizar la simulación eficientemente.

El core incluye:

- representación de celdas y estados de spin;
- construcción de sistemas y representación de la topología;
- dinámicas de Glauber y Metropolis;
- inicialización de estados;
- cálculo de magnetización, energía y correlación a primeros vecinos;
- cálculo de correlaciones espaciales a distancia `r`;
- cálculo de clusters geométricos y FK, y distribución de tamaños;
- ejecución de sweeps;
- serialización compacta del estado.

La intención es que esta parte pueda utilizarse para sistemas que no necesariamente sean grids.

### Experimentos sobre grids

`examples/common/simulacion_sqgrids.rs` contiene código específico para experimentos de ensamble sobre grids cuadrados (barridos de temperatura, réplicas, condiciones iniciales, fotografías del sistema y organización de resultados). Consume la API del core y sirve como ejemplo de cómo construir una aplicación experimental propia.

## Topología

El core no presupone que la red sea un grid.

Un sistema puede construirse a partir de una colección de relaciones entre elementos con `Sistema::from_edges`, que lee líneas `id_i,id_j`. La construcción de la topología determina explícitamente cuestiones como:

- si las relaciones son dirigidas o no dirigidas;
- si existen autoenlaces;
- cómo se identifican los nodos;
- cómo se compila la representación de identificadores a índices.

La conversión de identificadores a índices enteros permite que la representación utilizada durante la simulación sea eficiente sin perder una representación externa legible:

```text
identidad del elemento
        │
        ▼
      mapa
        │
        ▼
índice interno
        │
        ▼
   veclist
        │
        ▼
   topología usada
   por la simulación
```

La energía (`Sistema::energia`) supone que cada enlace aparece en la lista de vecinos de sus dos extremos (red no dirigida), como en `square_grid`.

## Estado y fotografías

El estado de una instancia puede representarse mediante `fotografia()`.

La fotografía utiliza una representación compacta basada en los índices donde cambia el estado del spin. El estado inicial de referencia es `Positivo`, y los índices almacenados representan las transiciones entre regiones de spins positivos y negativos. Por ejemplo, la fotografía `3 7 8 12` representa un estado cuyo spin cambia en esos índices.

La correspondencia entre índices e identidades se conserva mediante el mapa generado por `escribir_mapa()`, mientras que `escribir_red()` permite conservar la topología:

```text
mapa.txt     → identidad ↔ índice
red.txt      → topología
fotos/       → estado de cada sweep
series/      → observables
```

## Dinámica

Actualmente se implementan dos dinámicas, `Dinamica::Glauber` y `Dinamica::Metropolis`. Ambas operan sobre el mismo `Sistema`: la dinámica es un componente intercambiable del proceso de simulación y no una propiedad de la topología.

Un sweep consiste en N actualizaciones de sitios elegidos al azar:

```rust
for _ in 0..10_000 {
    sistema.sweep(&mut rng, &Dinamica::Glauber)?;
}
```

## Experimentos reproducibles

Todas las simulaciones reciben el generador aleatorio de forma explícita; con la misma semilla y los mismos parámetros, un experimento produce exactamente los mismos resultados. Los clusters FK consumen números aleatorios, así que medirlos cambia la trayectoria posterior respecto a un experimento que no los mida.

## Filosofía del proyecto

El objetivo no es construir una implementación limitada a una geometría particular, sino disponer de un núcleo suficientemente general para experimentar con distintas topologías y dinámicas.

La geometría, la forma de construir un experimento, la organización de los ensambles y la visualización deben permanecer fuera del núcleo siempre que sea posible:

```text
          aplicaciones
               │
       ┌───────┼────────┐
       │       │        │
    grids   visualización análisis
       │       │        │
       └───────┼────────┘
               │
               ▼
             core
               │
               ▼
          modelo Ising
```

El core proporciona el sistema y su dinámica; los consumidores deciden cómo construirlo, ejecutarlo, visualizarlo o analizarlo.

## Estado del proyecto

Proyecto experimental en desarrollo. La API y la organización interna pueden cambiar mientras se exploran distintas formas de representar sistemas de Ising sobre topologías generales.
