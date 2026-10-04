use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::Path;

use crate::simulaciones::CorrelacionMedia;


// ============================================================
// LECTURA DE LOS ARCHIVOS QUE ESCRIBE `Resultado::escribir`
// ============================================================

// Contenido de un directorio de salida. Cada parte es opcional; si un tipo de
// archivo aparece con varias cadencias (`serie_cada1.csv`, `serie_cada10.csv`),
// se toma el de menor cadencia.
pub struct Salida {
    pub parametros: Option<Parametros>,
    pub serie: Option<SerieLeida>,
    pub correlaciones: Option<Vec<CorrelacionMedia>>,
    pub clusters_geometricos: Option<ClustersLeidos>,
    pub clusters_fk: Option<ClustersLeidos>,
}

pub struct Parametros {
    pub n: usize,
    pub j: f64,
    pub h: f64,
    pub temp: f64,
    pub dinamica: String,
    pub burn_in: usize,
    pub mediciones: usize,
}

pub struct SerieLeida {
    pub sweeps: Vec<usize>,
    // (nombre de la métrica, valores), en el orden de las columnas.
    pub metricas: Vec<(String, Vec<f64>)>,
}

impl SerieLeida {
    pub fn metrica(&self, nombre: &str) -> Option<&[f64]> {
        self.metricas.iter().find(|(m, _)| m == nombre).map(|(_, v)| v.as_slice())
    }

    pub fn cada(&self) -> usize {
        if self.sweeps.len() > 1 {
            self.sweeps[1] - self.sweeps[0]
        } else {
            1
        }
    }
}

pub struct ClustersLeidos {
    // Por snapshot: sweep (si el archivo lo trae) y tamaños de mayor a menor.
    pub snapshots: Vec<(Option<usize>, Vec<usize>)>,
}

impl Salida {
    pub fn leer<P: AsRef<Path>>(directorio: P) -> Result<Self, Box<dyn Error>> {
        let dir = directorio.as_ref();

        // tipo -> [(cada, ruta)]
        let mut archivos: HashMap<&'static str, Vec<(usize, std::path::PathBuf)>> = HashMap::new();

        for entrada in fs::read_dir(dir)? {
            let ruta = entrada?.path();
            let Some(nombre) = ruta.file_name().and_then(|n| n.to_str()) else { continue };
            let Some(base) = nombre.strip_suffix(".csv") else { continue };

            for tipo in ["serie", "correlaciones", "clusters_geometricos", "clusters_fk"] {
                let cada = if base == tipo {
                    Some(0)
                } else {
                    base.strip_prefix(tipo)
                        .and_then(|r| r.strip_prefix("_cada"))
                        .and_then(|c| c.parse().ok())
                };

                if let Some(cada) = cada {
                    archivos.entry(tipo).or_default().push((cada, ruta.clone()));
                }
            }
        }

        let elegir = |tipo: &str| -> Option<std::path::PathBuf> {
            archivos.get(tipo)
                .and_then(|lista| lista.iter().min_by_key(|(c, _)| *c))
                .map(|(_, r)| r.clone())
        };

        let ruta_parametros = dir.join("parametros.csv");
        let parametros = if ruta_parametros.exists() {
            Some(leer_parametros(&fs::read_to_string(ruta_parametros)?)?)
        } else {
            None
        };

        let serie = elegir("serie").map(|r| fs::read_to_string(r)).transpose()?
            .map(|t| leer_serie(&t)).transpose()?;
        let correlaciones = elegir("correlaciones").map(|r| fs::read_to_string(r)).transpose()?
            .map(|t| leer_correlaciones(&t)).transpose()?;
        let clusters_geometricos = elegir("clusters_geometricos").map(|r| fs::read_to_string(r)).transpose()?
            .map(|t| leer_clusters(&t)).transpose()?;
        let clusters_fk = elegir("clusters_fk").map(|r| fs::read_to_string(r)).transpose()?
            .map(|t| leer_clusters(&t)).transpose()?;

        if serie.is_none() && correlaciones.is_none() && clusters_geometricos.is_none() && clusters_fk.is_none() {
            return Err(format!("No hay archivos de salida reconocibles en {}", dir.display()).into());
        }

        Ok(Salida {
            parametros,
            serie,
            correlaciones,
            clusters_geometricos,
            clusters_fk,
        })
    }

    // N del sistema: de los parámetros, o de la suma de tamaños de clusters de
    // un snapshot (los clusters cubren todos los sitios).
    pub fn n(&self) -> Option<usize> {
        if let Some(p) = &self.parametros {
            return Some(p.n);
        }

        self.clusters_geometricos.as_ref()
            .or(self.clusters_fk.as_ref())
            .and_then(|c| c.snapshots.first())
            .map(|(_, t)| t.iter().sum())
    }
}


// ============================================================
// CSV
// ============================================================

struct Tabla<'a> {
    columnas: Vec<&'a str>,
    filas: Vec<Vec<&'a str>>,
}

impl<'a> Tabla<'a> {
    fn new(texto: &'a str) -> Result<Self, Box<dyn Error>> {
        let mut lineas = texto.lines().map(str::trim).filter(|l| !l.is_empty());
        let columnas: Vec<&str> = lineas.next().ok_or("CSV vacío")?.split(',').collect();

        let mut filas = Vec::new();
        for linea in lineas {
            let fila: Vec<&str> = linea.split(',').collect();
            if fila.len() != columnas.len() {
                return Err(format!("Fila con {} campos, se esperaban {}: '{}'", fila.len(), columnas.len(), linea).into());
            }
            filas.push(fila);
        }

        Ok(Tabla { columnas, filas })
    }

    fn indice(&self, columna: &str) -> Option<usize> {
        self.columnas.iter().position(|c| *c == columna)
    }

    fn requerida(&self, columna: &str) -> Result<usize, Box<dyn Error>> {
        self.indice(columna).ok_or_else(|| format!("Falta la columna '{}'", columna).into())
    }
}

fn leer_parametros(texto: &str) -> Result<Parametros, Box<dyn Error>> {
    let t = Tabla::new(texto)?;
    let fila = t.filas.first().ok_or("parametros.csv sin datos")?;
    let campo = |c: &str| -> Result<&str, Box<dyn Error>> { Ok(fila[t.requerida(c)?]) };

    Ok(Parametros {
        n: campo("N")?.parse()?,
        j: campo("J")?.parse()?,
        h: campo("H")?.parse()?,
        temp: campo("T")?.parse()?,
        dinamica: campo("dinamica")?.to_string(),
        burn_in: campo("burn_in")?.parse()?,
        mediciones: campo("mediciones")?.parse()?,
    })
}

fn leer_serie(texto: &str) -> Result<SerieLeida, Box<dyn Error>> {
    let t = Tabla::new(texto)?;
    let i_sweep = t.requerida("sweep")?;

    let mut sweeps = Vec::with_capacity(t.filas.len());
    let mut metricas: Vec<(String, Vec<f64>)> = t.columnas.iter()
        .enumerate()
        .filter(|(i, _)| *i != i_sweep)
        .map(|(_, c)| (c.to_string(), Vec::with_capacity(t.filas.len())))
        .collect();

    for fila in &t.filas {
        sweeps.push(fila[i_sweep].parse()?);
        let mut k = 0;
        for (i, valor) in fila.iter().enumerate() {
            if i != i_sweep {
                metricas[k].1.push(valor.parse()?);
                k += 1;
            }
        }
    }

    Ok(SerieLeida { sweeps, metricas })
}

fn leer_correlaciones(texto: &str) -> Result<Vec<CorrelacionMedia>, Box<dyn Error>> {
    let t = Tabla::new(texto)?;
    let (ir, ip, ic, icc) = (t.requerida("r")?, t.requerida("pares")?, t.requerida("C")?, t.requerida("Cc")?);
    let ics = t.indice("Cc_por_snapshot");

    t.filas.iter().map(|f| {
        Ok(CorrelacionMedia {
            r: f[ir].parse()?,
            pares: f[ip].parse()?,
            c: f[ic].parse()?,
            cc: f[icc].parse()?,
            cc_por_snapshot: match ics {
                Some(i) => f[i].parse()?,
                None => f64::NAN,
            },
        })
    }).collect()
}

fn leer_clusters(texto: &str) -> Result<ClustersLeidos, Box<dyn Error>> {
    let t = Tabla::new(texto)?;
    let i_snap = t.requerida("snapshot")?;
    let i_tam = t.requerida("tamanio")?;
    let i_sweep = t.indice("sweep");

    let mut snapshots: Vec<(Option<usize>, Vec<usize>)> = Vec::new();
    let mut actual: Option<usize> = None;

    for fila in &t.filas {
        let snap: usize = fila[i_snap].parse()?;
        if actual != Some(snap) {
            let sweep = match i_sweep {
                Some(i) => Some(fila[i].parse()?),
                None => None,
            };
            snapshots.push((sweep, Vec::new()));
            actual = Some(snap);
        }
        if let Some((_, tamanios)) = snapshots.last_mut() {
            tamanios.push(fila[i_tam].parse()?);
        }
    }

    for (_, tamanios) in snapshots.iter_mut() {
        tamanios.sort_unstable_by(|a, b| b.cmp(a));
    }

    Ok(ClustersLeidos { snapshots })
}
