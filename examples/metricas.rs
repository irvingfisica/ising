use std::error::Error;
use std::fs::{self, File};
use std::io::{BufWriter, Write};

use rand::SeedableRng;
use rand::rngs::StdRng;

use ising::sistema::{Sistema, Inicial, Dinamica};

const L: usize = 32;

const J: f64 = 0.5;
const B: f64 = 1.0;
const TEMP: f64 = 1.0;

const BURN_IN: usize = 5_000;
const MEDICIONES: usize = 10_000;

const R_MAX: usize = 16;

const SEED: u64 = 123456789;


fn main() -> Result<(), Box<dyn Error>> {

    fs::create_dir_all("datos")?;

    let mut rng = StdRng::seed_from_u64(SEED);

    println!("Creando sistema...");

    let mut sistema = Sistema::square_grid(
        L,
        J,
        B,
        TEMP,
        Inicial::Random,
        &mut rng,
    );

    println!("Sistema: {} x {} = {} celdas", L, L, L * L);
    println!("J = {}", J);
    println!("B = {}", B);
    println!("T = {}", TEMP);

    println!("Burn-in: {} sweeps", BURN_IN);

    for _ in 0..BURN_IN {
        sistema.sweep(&mut rng, &Dinamica::Metropolis)?;
    }

    println!("Burn-in terminado.");
    println!("Midiendo {} sweeps...", MEDICIONES);

    let mut serie_m = Vec::with_capacity(MEDICIONES);
    let mut serie_abs_m = Vec::with_capacity(MEDICIONES);
    let mut serie_m2 = Vec::with_capacity(MEDICIONES);
    let mut serie_c1 = Vec::with_capacity(MEDICIONES);

    for sweep in 0..MEDICIONES {

        sistema.sweep(&mut rng, &Dinamica::Metropolis)?;

        let m = sistema.magnetizacion();

        serie_m.push(m);
        serie_abs_m.push(m.abs());
        serie_m2.push(m * m);
        serie_c1.push(sistema.c1());

        if sweep % 1000 == 0 {
            println!("  sweep {}", sweep);
        }
    }

    println!("Mediciones terminadas.");

    let resumen = calcular_resumen(
        &serie_m,
        &serie_abs_m,
        &serie_m2,
        &serie_c1,
    );

    println!();
    println!("========== RESUMEN ==========");
    println!("M medio       = {}", resumen.m_medio);
    println!("|M| medio     = {}", resumen.abs_m_medio);
    println!("M² medio      = {}", resumen.m2_medio);
    println!("Var(M)        = {}", resumen.var_m);
    println!("C1 medio      = {}", resumen.c1_medio);
    println!("tau_int(M)    = {}", resumen.tau_int);
    println!("==============================");

    escribir_serie(
        "datos/serie.csv",
        &serie_m,
        &serie_abs_m,
        &serie_m2,
        &serie_c1,
    )?;

    println!();
    println!("Calculando clusters geométricos...");

    let mut uf_geo = sistema.clusters_geometricos();

    let mut tamanios_geo = sistema.tamanios_clusters(&mut uf_geo);

    tamanios_geo.sort_unstable_by(|a, b| b.cmp(a));

    println!(
        "Clusters geométricos: {}",
        tamanios_geo.len()
    );

    println!(
        "Mayor cluster geométrico: {}",
        tamanios_geo.first().copied().unwrap_or(0)
    );

    escribir_clusters(
        "datos/clusters_geometricos.csv",
        &tamanios_geo,
    )?;

    println!();
    println!("Calculando clusters FK...");

    let mut uf_fk = sistema.clusters_fkw(&mut rng);

    let mut tamanios_fk = sistema.tamanios_clusters(&mut uf_fk);

    tamanios_fk.sort_unstable_by(|a, b| b.cmp(a));

    println!(
        "Clusters FK: {}",
        tamanios_fk.len()
    );

    println!(
        "Mayor cluster FK: {}",
        tamanios_fk.first().copied().unwrap_or(0)
    );

    escribir_clusters(
        "datos/clusters_fk.csv",
        &tamanios_fk,
    )?;

    let c1_snapshot = sistema.c1();

    let c1_r = sistema.correlacion_global_r(1);

    println!();
    println!("C1 snapshot = {}", c1_snapshot);
    println!("C(1) snapshot = {:?}", c1_r);

    println!();
    println!("Calculando correlaciones C(r)...");

    let m_snapshot = sistema.magnetizacion();
    let m2_snapshot = m_snapshot * m_snapshot;

    let correlaciones = sistema.correlaciones_global_r(R_MAX);

    escribir_correlaciones(
        "datos/correlaciones.csv",
        &correlaciones,
        sistema.magnetizacion(),
    )?;

    println!("Correlaciones:");

    println!("Correlaciones:");
    for (i, (pares, c)) in correlaciones.iter().enumerate() {
        let r = i + 1;
        let cc = *c - m2_snapshot;

        println!(
            "r = {:2}, pares = {:7}, C(r) = {:>12}, Cc(r) = {:>12}",
            r, pares, c, cc
        );
    }

    escribir_resumen(
        "datos/resumen.csv",
        &resumen,
        J,
        B,
        TEMP,
        L,
        SEED,
    )?;

    println!();
    println!("Experimento terminado.");

    Ok(())
}


struct Resumen {
    m_medio: f64,
    abs_m_medio: f64,
    m2_medio: f64,
    var_m: f64,
    c1_medio: f64,
    tau_int: f64,
}


fn calcular_resumen(
    m: &[f64],
    abs_m: &[f64],
    m2: &[f64],
    c1: &[f64],
) -> Resumen {

    let n = m.len() as f64;

    let m_medio = m.iter().sum::<f64>() / n;
    let abs_m_medio = abs_m.iter().sum::<f64>() / n;
    let m2_medio = m2.iter().sum::<f64>() / n;
    let c1_medio = c1.iter().sum::<f64>() / n;

    let var_m = m2_medio - m_medio * m_medio;

    let tau_int = autocorrelacion_integrada(m);

    Resumen {
        m_medio,
        abs_m_medio,
        m2_medio,
        var_m,
        c1_medio,
        tau_int,
    }
}


fn autocorrelacion_integrada(serie: &[f64]) -> f64 {

    if serie.len() < 2 {
        return 0.5;
    }

    let media =
        serie.iter().sum::<f64>() / serie.len() as f64;

    let varianza =
        serie.iter()
            .map(|x| {
                let d = x - media;
                d * d
            })
            .sum::<f64>()
            / serie.len() as f64;

    if varianza == 0.0 {
        return 0.5;
    }

    let max_lag = serie.len() / 2;

    let mut tau = 0.5;

    for lag in 1..max_lag {

        let mut suma = 0.0;
        let n = serie.len() - lag;

        for i in 0..n {
            suma +=
                (serie[i] - media)
                * (serie[i + lag] - media);
        }

        let c = suma / n as f64 / varianza;

        if c <= 0.0 {
            break;
        }

        tau += c;
    }

    tau
}


fn escribir_serie(
    ruta: &str,
    m: &[f64],
    abs_m: &[f64],
    m2: &[f64],
    c1: &[f64],
) -> Result<(), Box<dyn Error>> {

    let archivo = File::create(ruta)?;
    let mut w = BufWriter::new(archivo);

    writeln!(w, "sweep,M,abs_M,M2,C1")?;

    for i in 0..m.len() {
        writeln!(
            w,
            "{},{},{},{},{}",
            i,
            m[i],
            abs_m[i],
            m2[i],
            c1[i],
        )?;
    }

    Ok(())
}


fn escribir_clusters(
    ruta: &str,
    tamanios: &[usize],
) -> Result<(), Box<dyn Error>> {

    let archivo = File::create(ruta)?;
    let mut w = BufWriter::new(archivo);

    writeln!(w, "cluster,tamanio")?;

    for (i, tamanio) in tamanios.iter().enumerate() {
        writeln!(w, "{},{}", i, tamanio)?;
    }

    Ok(())
}


fn escribir_correlaciones(
    ruta: &str,
    correlaciones: &[(usize, f64)],
    m: f64,
) -> Result<(), Box<dyn Error>> {
    let archivo = File::create(ruta)?;
    let mut w = BufWriter::new(archivo);

    writeln!(w, "r,pares,C,Cc")?;

    let m2 = m * m;

    for (i, (pares, c)) in correlaciones.iter().enumerate() {
        let r = i + 1;
        let cc = *c - m2;

        writeln!(
            w,
            "{},{},{},{}",
            r, pares, c, cc
        )?;
    }

    Ok(())
}


fn escribir_resumen(
    ruta: &str,
    resumen: &Resumen,
    j: f64,
    b: f64,
    temp: f64,
    l: usize,
    seed: u64,
) -> Result<(), Box<dyn Error>> {

    let archivo = File::create(ruta)?;
    let mut w = BufWriter::new(archivo);

    writeln!(
        w,
        "L,J,B,T,seed,M,abs_M,M2,var_M,C1,tau_int"
    )?;

    writeln!(
        w,
        "{},{},{},{},{},{},{},{},{},{},{}",
        l,
        j,
        b,
        temp,
        seed,
        resumen.m_medio,
        resumen.abs_m_medio,
        resumen.m2_medio,
        resumen.var_m,
        resumen.c1_medio,
        resumen.tau_int,
    )?;

    Ok(())
}