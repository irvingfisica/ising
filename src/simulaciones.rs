use std::error::Error;
use std::fmt;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

use rand::Rng;

use crate::sistema::{Dinamica, Sistema};


// ============================================================
// DEFINICIÓN DEL EXPERIMENTO
// ============================================================

// Un experimento es: burn-in + `mediciones` sweeps de producción, durante los
// cuales cada plan mide sus métricas cada `cada` sweeps.
//
// Ejemplo (equivalente al experimento de atlas):
//
//     let experimento = Experimento::new(50_000, 50_000, Dinamica::Glauber)
//         .medir(PlanMedicion::cada(1, vec![
//             Metrica::Magnetizacion,
//             Metrica::MagnetizacionAbsoluta,
//             Metrica::C1,
//         ]))
//         .medir(PlanMedicion::cada(100, vec![
//             Metrica::Correlaciones { r_max: 64 },
//             Metrica::ClustersGeometricos,
//             Metrica::ClustersFK,
//         ]))
//         .progreso(1000);
//
//     let resultado = experimento.simular(&mut sistema, &mut rng)?;
//     println!("{}", resultado);
//     resultado.escribir("datos")?;
pub struct Experimento {
    pub burn_in: usize,
    pub mediciones: usize,
    pub dinamica: Dinamica,
    pub planes: Vec<PlanMedicion>,
    // Si es Some(k), imprime el avance cada k sweeps.
    pub progreso: Option<usize>,
}

impl Experimento {
    pub fn new(burn_in: usize, mediciones: usize, dinamica: Dinamica) -> Self {
        Self {
            burn_in,
            mediciones,
            dinamica,
            planes: Vec::new(),
            progreso: None,
        }
    }

    pub fn medir(mut self, plan: PlanMedicion) -> Self {
        self.planes.push(plan);
        self
    }

    pub fn progreso(mut self, cada: usize) -> Self {
        self.progreso = Some(cada);
        self
    }

    pub fn validar(&self) -> Result<(), Box<dyn Error>> {
        for (i, plan) in self.planes.iter().enumerate() {
            if plan.cada == 0 {
                return Err(format!("Plan {}: 'cada' debe ser mayor que cero", i).into());
            }

            for otro in &self.planes[i + 1..] {
                if otro.cada == plan.cada {
                    return Err(format!(
                        "Dos planes con la misma cadencia ({}); combínalos en uno solo",
                        plan.cada
                    ).into());
                }
            }

            for (k, metrica) in plan.metricas.iter().enumerate() {
                if let Metrica::Correlaciones { r_max: 0 } = metrica {
                    return Err(format!("Plan {}: Correlaciones requiere r_max > 0", i).into());
                }

                if plan.metricas[k + 1..].iter().any(|m| m.nombre() == metrica.nombre()) {
                    return Err(format!(
                        "Plan {}: la métrica '{}' aparece repetida",
                        i,
                        metrica.nombre()
                    ).into());
                }
            }
        }

        Ok(())
    }

    pub fn simular<R: Rng>(&self, sistema: &mut Sistema, rng: &mut R) -> Result<Resultado, Box<dyn Error>> {

        self.validar()?;

        for sweep in 0..self.burn_in {
            sistema.sweep(rng, &self.dinamica)?;
            self.reportar("burn-in", sweep);
        }

        let mut registros: Vec<RegistroPlan> = self.planes
            .iter()
            .map(|plan| RegistroPlan::nuevo(plan, self.mediciones))
            .collect();

        for sweep in 0..self.mediciones {
            sistema.sweep(rng, &self.dinamica)?;
            self.reportar("medición", sweep);

            let hechos = sweep + 1;

            for (plan, registro) in self.planes.iter().zip(registros.iter_mut()) {
                if hechos % plan.cada == 0 {
                    registro.medir(plan, hechos, sistema, rng);
                }
            }
        }

        Ok(Resultado {
            n: sistema.n(),
            j: sistema.j(),
            h: sistema.h(),
            temp: sistema.temp(),
            dinamica: self.dinamica,
            burn_in: self.burn_in,
            mediciones: self.mediciones,
            planes: registros,
        })
    }

    fn reportar(&self, fase: &str, sweep: usize) {
        if let Some(cada) = self.progreso {
            if cada > 0 && sweep % cada == 0 {
                println!("  {} sweep {}", fase, sweep);
            }
        }
    }
}

pub struct PlanMedicion {
    pub cada: usize,
    pub metricas: Vec<Metrica>,
}

impl PlanMedicion {
    pub fn cada(cada: usize, metricas: Vec<Metrica>) -> Self {
        assert!(cada > 0);

        Self {
            cada,
            metricas
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Metrica {
    Magnetizacion,
    MagnetizacionAbsoluta,
    Energia,
    C1,
    Correlaciones {
        r_max: usize
    },
    ClustersGeometricos,
    ClustersFK
}

impl Metrica {
    pub fn nombre(&self) -> &'static str {
        match self {
            Metrica::Magnetizacion => "M",
            Metrica::MagnetizacionAbsoluta => "abs_M",
            Metrica::Energia => "E",
            Metrica::C1 => "C1",
            Metrica::Correlaciones { .. } => "correlaciones",
            Metrica::ClustersGeometricos => "clusters_geometricos",
            Metrica::ClustersFK => "clusters_fk",
        }
    }

    pub fn es_escalar(&self) -> bool {
        matches!(
            self,
            Metrica::Magnetizacion | Metrica::MagnetizacionAbsoluta | Metrica::Energia | Metrica::C1
        )
    }
}


// ============================================================
// REGISTRO DE MEDICIONES
// ============================================================

pub struct SerieEscalar {
    pub metrica: &'static str,
    pub valores: Vec<f64>,
}

pub struct RegistroCorrelaciones {
    pub r_max: usize,
    // Magnetización de cada snapshot, para construir las correlaciones conectadas.
    pub m: Vec<f64>,
    // Por snapshot: (pares, C(r)) para r = 1..=r_max.
    pub snapshots: Vec<Vec<(usize, f64)>>,
}

// Todo lo medido por un plan. Los vectores están alineados con `sweeps`.
pub struct RegistroPlan {
    pub cada: usize,
    pub sweeps: Vec<usize>,
    pub escalares: Vec<SerieEscalar>,
    pub correlaciones: Option<RegistroCorrelaciones>,
    // Tamaños de clusters por snapshot, ordenados de mayor a menor.
    pub clusters_geometricos: Option<Vec<Vec<usize>>>,
    pub clusters_fk: Option<Vec<Vec<usize>>>,
}

impl RegistroPlan {
    fn nuevo(plan: &PlanMedicion, mediciones: usize) -> Self {
        let capacidad = mediciones / plan.cada;

        let mut registro = RegistroPlan {
            cada: plan.cada,
            sweeps: Vec::with_capacity(capacidad),
            escalares: Vec::new(),
            correlaciones: None,
            clusters_geometricos: None,
            clusters_fk: None,
        };

        for metrica in &plan.metricas {
            match metrica {
                Metrica::Correlaciones { r_max } => {
                    registro.correlaciones = Some(RegistroCorrelaciones {
                        r_max: *r_max,
                        m: Vec::with_capacity(capacidad),
                        snapshots: Vec::with_capacity(capacidad),
                    });
                },
                Metrica::ClustersGeometricos => registro.clusters_geometricos = Some(Vec::with_capacity(capacidad)),
                Metrica::ClustersFK => registro.clusters_fk = Some(Vec::with_capacity(capacidad)),
                _ => registro.escalares.push(SerieEscalar {
                    metrica: metrica.nombre(),
                    valores: Vec::with_capacity(capacidad),
                }),
            }
        }

        registro
    }

    fn medir<R: Rng>(&mut self, plan: &PlanMedicion, sweep: usize, sistema: &Sistema, rng: &mut R) {
        self.sweeps.push(sweep);

        // Los escalares se crearon en el mismo orden en que aparecen en el plan.
        let mut idx = 0;

        for metrica in &plan.metricas {
            match metrica {
                Metrica::Magnetizacion
                | Metrica::MagnetizacionAbsoluta
                | Metrica::Energia
                | Metrica::C1 => {
                    let valor = match metrica {
                        Metrica::Magnetizacion => sistema.magnetizacion(),
                        Metrica::MagnetizacionAbsoluta => sistema.magnetizacion().abs(),
                        Metrica::Energia => sistema.energia(),
                        _ => sistema.c1(),
                    };
                    self.escalares[idx].valores.push(valor);
                    idx += 1;
                },
                Metrica::Correlaciones { .. } => {
                    if let Some(reg) = self.correlaciones.as_mut() {
                        reg.m.push(sistema.magnetizacion());
                        reg.snapshots.push(sistema.correlaciones_global_r(reg.r_max));
                    }
                },
                Metrica::ClustersGeometricos => {
                    if let Some(reg) = self.clusters_geometricos.as_mut() {
                        let mut uf = sistema.clusters_geometricos();
                        reg.push(tamanios_ordenados(sistema, &mut uf));
                    }
                },
                Metrica::ClustersFK => {
                    if let Some(reg) = self.clusters_fk.as_mut() {
                        let mut uf = sistema.clusters_fkw(rng);
                        reg.push(tamanios_ordenados(sistema, &mut uf));
                    }
                },
            }
        }
    }
}

fn tamanios_ordenados(sistema: &Sistema, uf: &mut crate::sistema::UnionFind) -> Vec<usize> {
    let mut tamanios = sistema.tamanios_clusters(uf);
    tamanios.sort_unstable_by(|a, b| b.cmp(a));
    tamanios
}


// ============================================================
// RESULTADO Y ANÁLISIS
// ============================================================

pub struct Resultado {
    pub n: usize,
    pub j: f64,
    pub h: f64,
    pub temp: f64,
    pub dinamica: Dinamica,
    pub burn_in: usize,
    pub mediciones: usize,
    pub planes: Vec<RegistroPlan>,
}

// Estadística de una serie temporal. `tau_int` está en unidades de mediciones
// (no de sweeps) y `error` es el error estándar corregido por autocorrelación:
// sqrt(2 tau_int var / n).
#[derive(Debug, Clone)]
pub struct Estadistica {
    pub cada: usize,
    pub metrica: String,
    pub n: usize,
    pub media: f64,
    pub media2: f64,
    pub varianza: f64,
    pub tau_int: f64,
    pub error: f64,
}

impl Estadistica {
    pub fn de_serie(cada: usize, metrica: &str, serie: &[f64]) -> Self {
        let media = promedio(serie);
        let media2 = promedio_cuadrados(serie);
        let varianza = media2 - media * media;
        let tau_int = autocorrelacion_integrada(serie);
        let error = if serie.is_empty() {
            f64::NAN
        } else {
            (2.0 * tau_int * varianza.max(0.0) / serie.len() as f64).sqrt()
        };

        Self {
            cada,
            metrica: metrica.to_string(),
            n: serie.len(),
            media,
            media2,
            varianza,
            tau_int,
            error,
        }
    }
}

// Correlación promediada sobre snapshots para una distancia r.
//
// C   = <C(r)>                (promedio ponderado por pares)
// Cc  = <C(r)> - <m>²         (conectada del ensemble)
// Cc_por_snapshot = <C(r) - m²> = Cc - Var(m)
#[derive(Debug, Clone)]
pub struct CorrelacionMedia {
    pub r: usize,
    pub pares: usize,
    pub c: f64,
    pub cc: f64,
    pub cc_por_snapshot: f64,
}

#[derive(Debug, Clone)]
pub struct ResumenClusters {
    pub snapshots: usize,
    pub numero_medio: f64,
    pub mayor_medio: f64,
}

impl Resultado {

    pub fn plan(&self, cada: usize) -> Option<&RegistroPlan> {
        self.planes.iter().find(|p| p.cada == cada)
    }

    // Busca la serie de una métrica escalar en cualquier plan (la de menor
    // cadencia si aparece en varios).
    pub fn serie(&self, metrica: &Metrica) -> Option<&[f64]> {
        let mut planes: Vec<&RegistroPlan> = self.planes.iter().collect();
        planes.sort_by_key(|p| p.cada);

        planes.iter()
            .flat_map(|p| p.escalares.iter())
            .find(|s| s.metrica == metrica.nombre())
            .map(|s| s.valores.as_slice())
    }

    // Estadísticas de todas las series escalares, más el número de clusters y
    // el tamaño del mayor cluster por snapshot, tratados también como series.
    pub fn estadisticas(&self) -> Vec<Estadistica> {
        let mut resultado = Vec::new();

        for plan in &self.planes {
            for serie in &plan.escalares {
                resultado.push(Estadistica::de_serie(plan.cada, serie.metrica, &serie.valores));
            }

            for (prefijo, clusters) in [
                ("clusters_geometricos", &plan.clusters_geometricos),
                ("clusters_fk", &plan.clusters_fk),
            ] {
                if let Some(snapshots) = clusters {
                    let numero: Vec<f64> = snapshots.iter().map(|c| c.len() as f64).collect();
                    let mayor: Vec<f64> = snapshots.iter()
                        .map(|c| c.first().copied().unwrap_or(0) as f64)
                        .collect();

                    resultado.push(Estadistica::de_serie(plan.cada, &format!("{}_numero", prefijo), &numero));
                    resultado.push(Estadistica::de_serie(plan.cada, &format!("{}_mayor", prefijo), &mayor));
                }
            }
        }

        resultado
    }

    pub fn resumen_clusters(snapshots: &[Vec<usize>]) -> ResumenClusters {
        let numero: Vec<f64> = snapshots.iter().map(|c| c.len() as f64).collect();
        let mayor: Vec<f64> = snapshots.iter()
            .map(|c| c.first().copied().unwrap_or(0) as f64)
            .collect();

        ResumenClusters {
            snapshots: snapshots.len(),
            numero_medio: promedio(&numero),
            mayor_medio: promedio(&mayor),
        }
    }

    pub fn correlaciones_medias(registro: &RegistroCorrelaciones) -> Vec<CorrelacionMedia> {
        let n = registro.snapshots.len();

        if n == 0 {
            return Vec::new();
        }

        let m_medio = promedio(&registro.m);
        let m2_medio = promedio_cuadrados(&registro.m);
        let var_m = m2_medio - m_medio * m_medio;

        let mut resultado = Vec::with_capacity(registro.r_max);

        for ri in 0..registro.r_max {
            let mut suma_c = 0.0;
            let mut suma_pares = 0usize;

            for snapshot in &registro.snapshots {
                if let Some(&(pares, c)) = snapshot.get(ri) {
                    suma_c += c * pares as f64;
                    suma_pares += pares;
                }
            }

            let c = if suma_pares == 0 {
                0.0
            } else {
                suma_c / suma_pares as f64
            };

            let cc = c - m_medio * m_medio;

            resultado.push(CorrelacionMedia {
                r: ri + 1,
                pares: suma_pares / n,
                c,
                cc,
                cc_por_snapshot: cc - var_m,
            });
        }

        resultado
    }

    // --------------------------------------------------------
    // EXPORTACIÓN
    // --------------------------------------------------------

    // Escribe en `directorio`:
    //   parametros.csv                  N, J, H, T, dinámica, burn-in, mediciones
    //   resumen.csv                     estadísticas de cada serie (formato largo)
    //   serie.csv                       series escalares, una columna por métrica
    //   correlaciones.csv               r, pares, C, Cc, Cc_por_snapshot
    //   clusters_geometricos.csv        snapshot, sweep, cluster, tamanio
    //   clusters_fk.csv                 idem
    //
    // Si un mismo tipo de archivo lo producen varios planes, se agrega el
    // sufijo `_cada{k}` (p. ej. serie_cada1.csv, serie_cada10.csv).
    pub fn escribir<P: AsRef<Path>>(&self, directorio: P) -> Result<(), Box<dyn Error>> {
        let dir = directorio.as_ref();
        fs::create_dir_all(dir)?;

        self.escribir_parametros(&mut crear(dir, "parametros.csv")?)?;
        self.escribir_resumen(&mut crear(dir, "resumen.csv")?)?;

        let con_series = self.planes.iter().filter(|p| !p.escalares.is_empty()).count();
        let con_corr = self.planes.iter().filter(|p| p.correlaciones.is_some()).count();
        let con_geo = self.planes.iter().filter(|p| p.clusters_geometricos.is_some()).count();
        let con_fk = self.planes.iter().filter(|p| p.clusters_fk.is_some()).count();

        for plan in &self.planes {
            if !plan.escalares.is_empty() {
                let nombre = nombre_archivo("serie", plan.cada, con_series);
                escribir_serie(&mut crear(dir, &nombre)?, plan)?;
            }

            if let Some(reg) = &plan.correlaciones {
                let nombre = nombre_archivo("correlaciones", plan.cada, con_corr);
                escribir_correlaciones(&mut crear(dir, &nombre)?, &Self::correlaciones_medias(reg))?;
            }

            if let Some(snapshots) = &plan.clusters_geometricos {
                let nombre = nombre_archivo("clusters_geometricos", plan.cada, con_geo);
                escribir_clusters(&mut crear(dir, &nombre)?, &plan.sweeps, snapshots)?;
            }

            if let Some(snapshots) = &plan.clusters_fk {
                let nombre = nombre_archivo("clusters_fk", plan.cada, con_fk);
                escribir_clusters(&mut crear(dir, &nombre)?, &plan.sweeps, snapshots)?;
            }
        }

        Ok(())
    }

    pub fn escribir_parametros<W: Write>(&self, w: &mut W) -> Result<(), Box<dyn Error>> {
        writeln!(w, "N,J,H,T,dinamica,burn_in,mediciones")?;
        writeln!(
            w,
            "{},{},{},{},{:?},{},{}",
            self.n, self.j, self.h, self.temp, self.dinamica, self.burn_in, self.mediciones
        )?;

        Ok(())
    }

    pub fn escribir_resumen<W: Write>(&self, w: &mut W) -> Result<(), Box<dyn Error>> {
        writeln!(w, "cada,metrica,n,media,media2,varianza,tau_int,error")?;

        for e in self.estadisticas() {
            writeln!(
                w,
                "{},{},{},{},{},{},{},{}",
                e.cada, e.metrica, e.n, e.media, e.media2, e.varianza, e.tau_int, e.error
            )?;
        }

        Ok(())
    }
}

impl fmt::Display for Resultado {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "========== EXPERIMENTO ==========")?;
        writeln!(f, "N = {}, J = {}, H = {}, T = {}", self.n, self.j, self.h, self.temp)?;
        writeln!(
            f,
            "Dinámica = {:?}, burn-in = {}, mediciones = {}",
            self.dinamica, self.burn_in, self.mediciones
        )?;

        writeln!(f)?;
        writeln!(f, "---------- SERIES ----------")?;
        writeln!(
            f,
            "{:>5}  {:<28} {:>7} {:>14} {:>14} {:>14} {:>12}",
            "cada", "métrica", "n", "media", "error", "varianza", "tau_int"
        )?;

        for e in self.estadisticas() {
            writeln!(
                f,
                "{:>5}  {:<28} {:>7} {:>14.8} {:>14.8} {:>14.8} {:>12.3}",
                e.cada, e.metrica, e.n, e.media, e.error, e.varianza, e.tau_int
            )?;
        }

        for plan in &self.planes {
            if let Some(reg) = &plan.correlaciones {
                writeln!(f)?;
                writeln!(f, "---------- CORRELACIONES (cada {}, {} snapshots) ----------", plan.cada, reg.snapshots.len())?;

                for c in Self::correlaciones_medias(reg) {
                    writeln!(
                        f,
                        "r = {:3}, pares = {:8}, C(r) = {:>12.8}, Cc(r) = {:>12.8}",
                        c.r, c.pares, c.c, c.cc
                    )?;
                }
            }

            for (nombre, clusters) in [
                ("geométricos", &plan.clusters_geometricos),
                ("FK", &plan.clusters_fk),
            ] {
                if let Some(snapshots) = clusters {
                    let r = Self::resumen_clusters(snapshots);
                    writeln!(f)?;
                    writeln!(f, "---------- CLUSTERS {} (cada {}, {} snapshots) ----------", nombre, plan.cada, r.snapshots)?;
                    writeln!(f, "Número medio de clusters       = {}", r.numero_medio)?;
                    writeln!(f, "Tamaño medio del mayor cluster = {}", r.mayor_medio)?;
                }
            }
        }

        write!(f, "=================================")
    }
}

fn crear(dir: &Path, nombre: &str) -> Result<BufWriter<File>, Box<dyn Error>> {
    Ok(BufWriter::new(File::create(dir.join(nombre))?))
}

fn nombre_archivo(base: &str, cada: usize, planes_con_tipo: usize) -> String {
    if planes_con_tipo > 1 {
        format!("{}_cada{}.csv", base, cada)
    } else {
        format!("{}.csv", base)
    }
}

fn escribir_serie<W: Write>(w: &mut W, plan: &RegistroPlan) -> Result<(), Box<dyn Error>> {
    write!(w, "sweep")?;
    for serie in &plan.escalares {
        write!(w, ",{}", serie.metrica)?;
    }
    writeln!(w)?;

    for (i, sweep) in plan.sweeps.iter().enumerate() {
        write!(w, "{}", sweep)?;
        for serie in &plan.escalares {
            write!(w, ",{}", serie.valores[i])?;
        }
        writeln!(w)?;
    }

    Ok(())
}

fn escribir_correlaciones<W: Write>(w: &mut W, correlaciones: &[CorrelacionMedia]) -> Result<(), Box<dyn Error>> {
    writeln!(w, "r,pares,C,Cc,Cc_por_snapshot")?;

    for c in correlaciones {
        writeln!(w, "{},{},{},{},{}", c.r, c.pares, c.c, c.cc, c.cc_por_snapshot)?;
    }

    Ok(())
}

fn escribir_clusters<W: Write>(w: &mut W, sweeps: &[usize], snapshots: &[Vec<usize>]) -> Result<(), Box<dyn Error>> {
    writeln!(w, "snapshot,sweep,cluster,tamanio")?;

    for (s, (sweep, clusters)) in sweeps.iter().zip(snapshots).enumerate() {
        for (i, tam) in clusters.iter().enumerate() {
            writeln!(w, "{},{},{},{}", s, sweep, i, tam)?;
        }
    }

    Ok(())
}


// ============================================================
// ESTADÍSTICA DE SERIES
// ============================================================

pub fn promedio(datos: &[f64]) -> f64 {
    if datos.is_empty() {
        return f64::NAN;
    }

    datos.iter().sum::<f64>() / datos.len() as f64
}

pub fn promedio_cuadrados(datos: &[f64]) -> f64 {
    if datos.is_empty() {
        return f64::NAN;
    }

    datos.iter().map(|x| x * x).sum::<f64>() / datos.len() as f64
}

// rho(t) para t = 0..max_lag (acotado a n/2), con la misma normalización que
// autocorrelacion_integrada.
pub fn autocorrelacion(serie: &[f64], max_lag: usize) -> Vec<f64> {
    let n = serie.len();
    if n < 2 {
        return vec![1.0];
    }

    let media = promedio(serie);
    let varianza = serie.iter().map(|x| (x - media) * (x - media)).sum::<f64>() / n as f64;

    if varianza == 0.0 {
        return vec![1.0];
    }

    let lags = max_lag.min(n / 2);
    let mut rho = Vec::with_capacity(lags + 1);

    for lag in 0..=lags {
        let m = n - lag;
        let mut suma = 0.0;
        for i in 0..m {
            suma += (serie[i] - media) * (serie[i + lag] - media);
        }
        rho.push(suma / m as f64 / varianza);
    }

    rho
}

// tau_int = 1/2 + sum_{t>=1} rho(t), cortando la suma en el primer rho(t) <= 0.
pub fn autocorrelacion_integrada(serie: &[f64]) -> f64 {
    if serie.len() < 2 {
        return 0.5;
    }

    let media = promedio(serie);

    let varianza = serie.iter()
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
        let n = serie.len() - lag;

        let mut suma = 0.0;

        for i in 0..n {
            suma += (serie[i] - media) * (serie[i + lag] - media);
        }

        let c = suma / n as f64 / varianza;

        if c <= 0.0 {
            break;
        }

        tau += c;
    }

    tau
}
