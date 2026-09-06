# Ising

Implementación experimental de un modelo de Ising en Rust.

El proyecto está diseñado alrededor de una separación entre el **modelo genérico del sistema** y las implementaciones concretas utilizadas para realizar experimentos. El núcleo no supone una geometría particular: un sistema puede definirse sobre una topología arbitraria.

Actualmente se incluye, además del núcleo, una implementación para realizar experimentos de ensamble sobre grids cuadrados.

## Arquitectura

La arquitectura puede entenderse en dos niveles:

```text
                 ising
                   │
          ┌────────┴────────┐
          │                 │
         core          experimentos
          │                 │
          │          simulacion_sqgrids
          │                 │
          │          grids cuadrados
          │          ensambles
          │          réplicas
          │          resultados
          │
          ├── Sistema
          ├── Celda
          ├── Estado
          ├── Dinamica
          ├── Inicial
          └── topología
```

### Core

El núcleo contiene la representación y dinámica de un sistema de Ising sin imponer una geometría específica.

Un `Sistema` está compuesto por elementos (`Celda`) y una topología que determina las relaciones entre ellos. La topología puede representarse mediante identificadores y posteriormente compilarse a índices enteros para realizar la simulación eficientemente.

El core incluye:

- representación de celdas;
- estados de spin;
- construcción de sistemas;
- representación de la topología;
- dinámica de Glauber;
- dinámica de Metropolis;
- inicialización de estados;
- cálculo de magnetización;
- ejecución de sweeps;
- serialización compacta del estado.

La intención es que esta parte pueda utilizarse para sistemas que no necesariamente sean grids.

### Experimentos sobre grids

`simulacion_sqgrids` contiene código específico para realizar experimentos sobre grids cuadrados.

Esta parte no define el modelo de Ising. Consume la API del core para construir experimentos concretos, por ejemplo:

- construcción de grids cuadrados;
- barridos de temperatura;
- ensambles de réplicas;
- condiciones iniciales;
- períodos de *burning*;
- series temporales;
- almacenamiento de fotografías del sistema;
- organización de resultados.

Esta implementación también funciona como ejemplo de cómo construir una aplicación experimental utilizando el core.

## Estructura del proyecto

```text
src/
├── lib.rs
├── sistema.rs
├── simulacion_sqgrids.rs
└── bin/
    ├── ensamble_grid.rs
    ├── ensamble_grid_meta.rs
    └── simular_grid.rs
```

### `sistema.rs`

Contiene el núcleo del modelo.

Entre las principales abstracciones se encuentran:

```rust
Sistema
Celda
Estado
Dinamica
Inicial
```

`Sistema` mantiene tanto los elementos del sistema como su topología y los parámetros físicos de la simulación.

### `simulacion_sqgrids.rs`

Implementa los experimentos específicos utilizados actualmente para estudiar el modelo sobre un grid cuadrado.

El módulo utiliza `Sistema::square_grid()` para construir la topología y posteriormente ejecuta las dinámicas definidas por el core.

### `bin/`

Los binarios definen experimentos concretos y parámetros de ejecución, mientras que la lógica reutilizable permanece en la librería.

Por ejemplo:

```rust
let mut sistema = Sistema::square_grid(
    L,
    1.0,
    0.0,
    temperatura,
    inicial,
    &mut rng,
);
```

y posteriormente:

```rust
sistema.sweep(&mut rng, &dinamica)?;
```

La idea es que el binario determine **qué experimento realizar**, mientras que la librería contiene **cómo funciona el sistema**.

## Topología

El core no presupone que la red sea un grid.

Un sistema puede construirse a partir de una colección de relaciones entre elementos. La construcción de la topología determina explícitamente cuestiones como:

- si las relaciones son dirigidas o no dirigidas;
- si existen autoenlaces;
- cómo se identifican los nodos;
- cómo se compila la representación de identificadores a índices.

La conversión de identificadores a índices enteros permite que la representación utilizada durante la simulación sea eficiente sin perder una representación externa legible.

En particular, se mantiene la separación:

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

## Estado y fotografías

El estado de una instancia puede representarse mediante `fotografia()`.

La fotografía utiliza una representación compacta basada en los índices donde cambia el estado del spin. El estado inicial de referencia es `Positivo`, y los índices almacenados representan las transiciones entre regiones de spins positivos y negativos.

Por ejemplo, una fotografía como:

```text
3 7 8 12
```

representa un estado cuyo spin cambia en esos índices.

La representación permite almacenar series de estados sin escribir explícitamente un valor para cada celda.

La correspondencia entre índices e identidades se conserva mediante el mapa generado por `escribir_mapa()`, mientras que `escribir_red()` permite conservar la topología.

De esta manera, una ejecución puede representarse conceptualmente mediante:

```text
mapa.txt     → identidad ↔ índice
red.txt      → topología
fotos/       → estado de cada sweep
series/      → observables
```

## Dinámica

Actualmente se implementan dos dinámicas:

```rust
Dinamica::Glauber
Dinamica::Metropolis
```

Ambas operan sobre el mismo `Sistema`. La dinámica es, por tanto, un componente intercambiable del proceso de simulación y no una propiedad intrínseca de la topología.

El flujo general es:

```text
Sistema
   │
   ├── estado
   ├── topología
   └── parámetros
         │
         ▼
      dinámica
         │
         ▼
       sweep
         │
         ▼
   nuevo estado
```

## Experimentos reproducibles

Las simulaciones experimentales utilizan semillas derivadas de los parámetros de la ejecución y del número de réplica.

Los resultados de una ejecución se almacenan en una carpeta propia, incluyendo la información necesaria para identificar la configuración y reconstruir los resultados.

La organización concreta de los experimentos pertenece a la capa experimental y no al core.

## Uso

El crate puede utilizarse como librería:

```rust
use ising::sistema::{
    Sistema,
    Dinamica,
    Inicial,
};
```

Un uso básico consiste en construir un sistema, seleccionar una dinámica y ejecutar sweeps:

```rust
let mut sistema = Sistema::square_grid(
    100,
    1.0,
    0.0,
    2.269185,
    Inicial::Random,
    &mut rng,
);

let dinamica = Dinamica::Glauber;

for _ in 0..10_000 {
    sistema.sweep(&mut rng, &dinamica)?;
}
```

Los experimentos más elaborados pueden construirse sobre esta misma API.

## Filosofía del proyecto

El objetivo no es construir una implementación limitada a una geometría particular, sino disponer de un núcleo suficientemente general para experimentar con distintas topologías y dinámicas.

La geometría, la forma de construir un experimento, la organización de los ensambles y la visualización deben permanecer fuera del núcleo siempre que sea posible.

Por ello, la arquitectura sigue esta dirección:

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

Proyecto experimental en desarrollo.

La API y la organización interna pueden cambiar mientras se exploran distintas formas de representar sistemas de Ising sobre topologías generales.

La implementación de grids cuadrados constituye actualmente el principal entorno de experimentación, pero no define las capacidades fundamentales del núcleo.