use std::error::Error;
use std::writeln;
use std::io::{BufWriter, Write};
use std::fs::{self,File};
use rand::SeedableRng;
use rand::rngs::StdRng;
use rayon::prelude::*;
use chrono::Local;
use std::path::{Path,PathBuf};

use crate::sistema::{Sistema, Inicial, Dinamica};

const L: usize = 100;
const N_REPLICAS: usize = 100;
const N_BURNING: usize = 30_000;
//const N_BURNING: usize = 0;
const N_SWEEPS: usize = 10_000;

const TC: f64 = 2.269185;

pub fn condiciones(n_experimentos: usize) {
    println!(
        "Tamaño del grid: {}",
        L
    );

    println!("Experimentos: {}", n_experimentos);

    println!(
        "Réplicas por experimento: {}",
        N_REPLICAS
    );

    println!(
        "Simulaciones totales: {}",
        n_experimentos * N_REPLICAS
    );

    println!(
        "Burn-in: {} sweeps",
        N_BURNING
    );

    println!(
        "Producción: {} sweeps",
        N_SWEEPS
    );

    println!();
}

pub fn construir_temps() -> Vec<f64> {
    let mut temperaturas = Vec::new();

    let mut t = 1.5;
    while t < 2.0 {
        temperaturas.push(t);
        t += 0.025;
    }

    t = 2.0;
    while t < 2.5 {
        temperaturas.push(t);
        t += 0.005;
    }

    t = 2.6;
    while t <= 4.0 {
        temperaturas.push(t);
        t += 0.1;
    }

    temperaturas.push(TC);

    temperaturas.sort_by(|a, b| a.partial_cmp(b).unwrap());

    temperaturas.dedup_by(|a, b| (*a - *b).abs() < 1e-10);

    temperaturas
}

pub fn construir_temps_propors() -> Vec<(f64,f32)> {
    let mut tuplas = Vec::new();

    let mut t = 1.5;

    while t < 2.0 {
        for p in (0..=20).map(|i| i as f32 / 20.0) {
            tuplas.push((t,p));
        }
        t += 0.1;
    }

    t = 2.0;
    while t < TC {
        for p in (0..=20).map(|i| i as f32 / 20.0) {
            tuplas.push((t,p));
        }
        t += 0.05;
    }

    tuplas
}

pub fn crear_carpeta_ejecucion() -> Result<PathBuf, Box<dyn Error + Send + Sync>> {
    let marca = Local::now().format("%Y-%m-%d_%H-%M-%S-%3f");

    let carpeta = PathBuf::from(
        format!("resultados/{}",marca)
    );

    fs::create_dir_all(carpeta.join("series"))?;
    fs::create_dir_all(carpeta.join("fotos"))?;

    Ok(carpeta)
}

pub fn crear_sistema_base(carpeta: &Path) -> Result<(), Box<dyn Error + Send + Sync>> {

    let mut rng = rand::rng();

    let sistema = Sistema::square_grid(
        L,
        1.0,
        0.0,
        0.0,
        Inicial::Random,
        &mut rng,
    );

    let archivo_mapa = File::create(carpeta.join("mapa.txt"))?;
    let mut mapa = BufWriter::new(archivo_mapa);

    let archivo_red = File::create(carpeta.join("red.txt"))?;
    let mut red = BufWriter::new(archivo_red);

    sistema.escribir_mapa(&mut mapa)?;
    sistema.escribir_red(&mut red)?;

    Ok(())
}

fn core_simulation(mut sistema: Sistema, mut rng: StdRng, dinamica: &Dinamica) -> Result<(Vec<f64>,String), Box<dyn Error + Send + Sync>> {
    for _ in 0..N_BURNING {
        sistema.sweep(
            &mut rng,
            dinamica,
        ).map_err(|e| format!("{e}"))?;
    }

    let mut serie = Vec::with_capacity(N_SWEEPS);

    for _ in 0..N_SWEEPS {

        sistema.sweep(
            &mut rng,
            dinamica,
        ).map_err(|e| format!("{e}"))?;

        serie.push(sistema.magnetizacion());
    }

    let fotografia = sistema.fotografia();

    Ok((serie,fotografia))
}

fn simular_meta(temperatura: f64, proporcion_positivos: f32, replica: usize) -> Result<(Vec<f64>,String), Box<dyn Error + Send + Sync>> {
    let seed = generar_seed(temperatura, proporcion_positivos, replica);

    let mut rng = StdRng::seed_from_u64(seed);

    let inicial = Inicial::Parcial(proporcion_positivos);

    let sistema = Sistema::square_grid(
        L,
        1.0,
        0.0,
        temperatura,
        inicial,
        &mut rng,
    );

    let (serie,fotografia) = core_simulation(sistema, rng, &Dinamica::Glauber)?;

    Ok((serie,fotografia))
}

fn simular(temperatura: f64, replica: usize) -> Result<(Vec<f64>,String), Box<dyn Error + Send + Sync>> {
    let seed = generar_seed(temperatura, 0.0, replica);

    let mut rng = StdRng::seed_from_u64(seed);

    let inicial = if temperatura < TC {
        Inicial::Positivo
    } else {
        Inicial::Random
    };

    let sistema = Sistema::square_grid(
        L,
        1.0,
        0.0,
        temperatura,
        inicial,
        &mut rng,
    );

    let (serie,fotografia) = core_simulation(sistema, rng, &Dinamica::Glauber)?;

    Ok((serie,fotografia))

}

pub fn simular_instancia(temperatura: f64, inicial: Inicial, replica: usize, carpeta: &Path) -> Result<(), Box<dyn Error + Send + Sync>> {
    let seed = generar_seed(temperatura, 0.0, replica);

    let mut rng = StdRng::seed_from_u64(seed);

    let nombre = carpeta.join("series")
        .join(format!("T_{:.6}.txt",temperatura));

    let nombre_fotos = carpeta.join("fotos")
        .join(format!("T_{:.6}.txt",temperatura));

    let archivo = File::create(nombre)?;
    let mut writer = BufWriter::new(archivo);

    let archivo_fotos = File::create(nombre_fotos)?;
    let mut writer_fotos = BufWriter::new(archivo_fotos);

    let mut sistema = Sistema::square_grid(
        L,
        1.0,
        0.0,
        temperatura,
        inicial,
        &mut rng,
    );

    let dinamica = &Dinamica::Glauber;

    writeln!(writer,"replica,t,M")?;

    for _ in 0..N_BURNING {
        sistema.sweep(
            &mut rng,
            dinamica,
        ).map_err(|e| format!("{e}"))?;
    }

    for t in 0..N_SWEEPS {

        sistema.sweep(&mut rng, dinamica).map_err(|e| format!("{e}"))?;

        let magnetizacion = sistema.magnetizacion();
        let fotografia = sistema.fotografia();

        writeln!(
                writer,
                "{},{},{:.12}",
                replica,
                t,
                magnetizacion
            )?;

        writeln!(
            writer_fotos,
            "{}",
            fotografia
        )?;

    };

    Ok(())
}

pub fn simular_temperatura(temperatura: f64, carpeta: &Path) -> Result<(), Box<dyn Error + Send + Sync>> {

    let nombre = carpeta.join("series")
        .join(format!("T_{:.6}.txt",temperatura));

    let nombre_fotos = carpeta.join("fotos")
        .join(format!("T_{:.6}.txt",temperatura));

    let archivo = File::create(nombre)?;
    let mut writer = BufWriter::new(archivo);

    let archivo_fotos = File::create(nombre_fotos)?;
    let mut writer_fotos = BufWriter::new(archivo_fotos);

    writeln!(writer,"replica,t,M")?;

    let resultados: Vec<_> = (0..N_REPLICAS).into_par_iter()
        .map(|replica| {
            let (serie,fotografia) = simular(temperatura, replica)?;

            Ok::<_,Box<dyn Error + Send + Sync>>((replica,serie,fotografia))
        }).collect::<Result<Vec<_>,_>>()?;

    for (replica, serie,_) in &resultados {

        for (t, magnetizacion) in serie.iter().enumerate() {

            writeln!(
                writer,
                "{},{},{:.12}",
                replica,
                t,
                magnetizacion
            )?;
        }
    }

    for (_, _, fotografia) in &resultados {

        writeln!(
            writer_fotos,
            "{}",
            fotografia
        )?;
    }

    Ok(())
}

pub fn simular_temperatura_proporcion(temperatura: f64, proporcion_positivos: f32, carpeta: &Path) -> Result<(), Box<dyn Error + Send + Sync>> {
    let nombre = carpeta.join("series").join(
        format!(
            "T_{:.6}_{:.2}.csv",
            temperatura,
            proporcion_positivos
        )
    );

    let nombre_fotos = carpeta.join("fotos").join(
        format!(
            "T_{:.6}_{:.2}.txt",
            temperatura,
            proporcion_positivos
        )
    );

    let archivo = File::create(nombre)?;
    let mut writer = BufWriter::new(archivo);

    let archivo_fotos = File::create(nombre_fotos)?;
    let mut writer_fotos = BufWriter::new(archivo_fotos);

    writeln!(writer,"replica,t,M")?;

    let resultados: Vec<_> = (0..N_REPLICAS).into_par_iter()
        .map(|replica| {
            let (serie,fotografia) = simular_meta(temperatura, proporcion_positivos, replica)?;

            Ok::<_,Box<dyn Error + Send + Sync>>((replica,serie,fotografia))
        }).collect::<Result<Vec<_>,_>>()?;

    for (replica, serie,_) in &resultados {

        for (t, magnetizacion) in serie.iter().enumerate() {

            writeln!(
                writer,
                "{},{},{:.12}",
                replica,
                t,
                magnetizacion
            )?;
        }
    }

    for (_, _, fotografia) in &resultados {

        writeln!(
            writer_fotos,
            "{}",
            fotografia
        )?;
    }

    Ok(())
}

fn generar_seed(
    temperatura: f64,
    proporcion_positivos: f32,
    replica: usize,
) -> u64 {
    let proporcion_id = (proporcion_positivos * 100.0).round() as u64;

    let mut seed =
        temperatura.to_bits()
        ^ proporcion_id.wrapping_mul(0x9E3779B97F4A7C15)
        ^ (replica as u64).wrapping_mul(0xBF58476D1CE4E5B9);

    seed ^= seed >> 30;
    seed = seed.wrapping_mul(0xBF58476D1CE4E5B9);
    seed ^= seed >> 27;
    seed = seed.wrapping_mul(0x94D049BB133111EB);
    seed ^= seed >> 31;

    seed
}