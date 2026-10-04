use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use plotters::prelude::*;

use crate::salida::{ClustersLeidos, Salida};
use crate::simulaciones::{autocorrelacion, autocorrelacion_integrada, promedio, CorrelacionMedia};


// ============================================================
// GRÁFICAS ESTÁTICAS (SVG) DE UN DIRECTORIO DE SALIDA
// ============================================================

// Lee los CSV de `datos` (ver `Resultado::escribir`) y escribe en `destino`:
//
//   serie_{metrica}.svg            traza de cada métrica escalar
//   histograma_{metrica}.svg       histograma de la magnetización (o la 1a métrica)
//   autocorrelacion_{metrica}.svg  rho(t), con tau_int
//   correlaciones.svg              C(r) y Cc(r) en log-log, con r^(-1/4)
//   clusters_distribucion.svg      n(s) en log-log, con s^(-tau) de Fisher
//   clusters_mayor.svg             fracción del mayor cluster por snapshot
//   clusters_numero.svg            número de clusters por snapshot
//
// Devuelve las rutas escritas.
pub fn graficar<P: AsRef<Path>, Q: AsRef<Path>>(datos: P, destino: Q) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let salida = Salida::leer(datos)?;
    let dir = destino.as_ref();
    fs::create_dir_all(dir)?;

    let mut escritas = Vec::new();

    if let Some(serie) = &salida.serie {
        let cada = serie.cada();
        for (nombre, valores) in &serie.metricas {
            let ruta = dir.join(format!("serie_{}.svg", nombre));
            traza(&ruta, nombre, &serie.sweeps, valores)?;
            escritas.push(ruta);
        }

        let principal = serie.metricas.iter()
            .find(|(n, _)| n == "M")
            .or(serie.metricas.first());

        if let Some((nombre, valores)) = principal {
            let ruta = dir.join(format!("histograma_{}.svg", nombre));
            histograma(&ruta, nombre, valores, 40)?;
            escritas.push(ruta);

            let ruta = dir.join(format!("autocorrelacion_{}.svg", nombre));
            grafica_autocorrelacion(&ruta, nombre, valores, cada)?;
            escritas.push(ruta);
        }
    }

    if let Some(correlaciones) = &salida.correlaciones {
        let ruta = dir.join("correlaciones.svg");
        grafica_correlaciones(&ruta, correlaciones)?;
        escritas.push(ruta);
    }

    let n = salida.n().unwrap_or(1);
    let clusters: Vec<(&str, &ClustersLeidos, RGBColor, f64)> = [
        ("Geométricos", salida.clusters_geometricos.as_ref(), SERIE_1, 379.0 / 187.0),
        ("FK", salida.clusters_fk.as_ref(), SERIE_2, 31.0 / 15.0),
    ]
        .into_iter()
        .filter_map(|(nombre, c, color, tau)| c.map(|c| (nombre, c, color, tau)))
        .filter(|(_, c, _, _)| !c.snapshots.is_empty())
        .collect();

    if !clusters.is_empty() {
        let ruta = dir.join("clusters_distribucion.svg");
        distribucion_clusters(&ruta, &clusters, n)?;
        escritas.push(ruta);

        let ruta = dir.join("clusters_mayor.svg");
        serie_clusters(&ruta, "Fracción del mayor cluster", "s_max / N", &clusters,
            |t| t.first().copied().unwrap_or(0) as f64 / n as f64)?;
        escritas.push(ruta);

        let ruta = dir.join("clusters_numero.svg");
        serie_clusters(&ruta, "Número de clusters", "clusters", &clusters, |t| t.len() as f64)?;
        escritas.push(ruta);
    }

    Ok(escritas)
}


// ============================================================
// ESTILO
// ============================================================

const TAMANIO: (u32, u32) = (800, 480);
const FUENTE: &str = "sans-serif";

const FONDO: RGBColor = RGBColor(0xfc, 0xfc, 0xfb);
const TINTA: RGBColor = RGBColor(0x0b, 0x0b, 0x0b);
const TINTA_2: RGBColor = RGBColor(0x52, 0x51, 0x4e);
const TENUE: RGBColor = RGBColor(0x89, 0x87, 0x81);
const REJILLA: RGBColor = RGBColor(0xe1, 0xe0, 0xd9);
const EJE: RGBColor = RGBColor(0xc3, 0xc2, 0xb7);
const SERIE_1: RGBColor = RGBColor(0x2a, 0x78, 0xd6);
const SERIE_2: RGBColor = RGBColor(0xeb, 0x68, 0x34);

type Resultado<T> = Result<T, Box<dyn Error>>;

fn lienzo(ruta: &Path) -> DrawingArea<SVGBackend<'_>, plotters::coord::Shift> {
    SVGBackend::new(ruta, TAMANIO).into_drawing_area()
}

macro_rules! malla {
    ($chart:expr, $x:expr, $y:expr) => {
        $chart.configure_mesh()
            .x_desc($x)
            .y_desc($y)
            .x_label_formatter(&|v| numero(*v))
            .y_label_formatter(&|v| numero(*v))
            .bold_line_style(REJILLA.stroke_width(1))
            .light_line_style(TRANSPARENT)
            .axis_style(EJE.stroke_width(1))
            .label_style((FUENTE, 13).into_font().color(&TENUE))
            .axis_desc_style((FUENTE, 14).into_font().color(&TINTA_2))
            .draw()?
    };
}

macro_rules! leyenda {
    ($chart:expr) => {
        $chart.configure_series_labels()
            .position(SeriesLabelPosition::UpperRight)
            .background_style(FONDO.mix(0.9))
            .border_style(REJILLA)
            .label_font((FUENTE, 13).into_font().color(&TINTA_2))
            .draw()?
    };
}

// Etiquetas de ejes: sin ceros sobrantes; notación científica para valores
// muy grandes o muy pequeños (1e-5).
fn numero(v: f64) -> String {
    let a = v.abs();
    if v == 0.0 {
        return "0".to_string();
    }
    if a >= 1e5 || a < 1e-3 {
        let e = a.log10().floor() as i32;
        let m = v / 10f64.powi(e);
        if (m.abs() - 1.0).abs() < 1e-9 {
            return format!("{}1e{}", if v < 0.0 { "-" } else { "" }, e);
        }
        return format!("{}e{}", recortar(format!("{:.2}", m)), e);
    }
    recortar(format!("{:.4}", v))
}

fn recortar(s: String) -> String {
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

fn clave_linea(color: RGBColor) -> impl Fn((i32, i32)) -> PathElement<(i32, i32)> {
    move |(x, y)| PathElement::new(vec![(x, y), (x + 18, y)], color.stroke_width(2))
}

fn clave_referencia() -> impl Fn((i32, i32)) -> PathElement<(i32, i32)> {
    |(x, y)| PathElement::new(vec![(x, y), (x + 18, y)], TENUE.stroke_width(1))
}

fn extension(valores: impl Iterator<Item = f64>) -> (f64, f64) {
    let (mut a, mut b) = (f64::INFINITY, f64::NEG_INFINITY);
    for v in valores.filter(|v| v.is_finite()) {
        a = a.min(v);
        b = b.max(v);
    }
    if !a.is_finite() {
        return (0.0, 1.0);
    }
    if a == b {
        return (a - 0.5, b + 0.5);
    }
    let margen = (b - a) * 0.04;
    (a - margen, b + margen)
}

fn extension_log(valores: impl Iterator<Item = f64>) -> (f64, f64) {
    let (mut a, mut b) = (f64::INFINITY, f64::NEG_INFINITY);
    for v in valores.filter(|v| v.is_finite() && *v > 0.0) {
        a = a.min(v);
        b = b.max(v);
    }
    if !a.is_finite() {
        return (1.0, 10.0);
    }
    (10f64.powf(a.log10().floor()), 10f64.powf(b.log10().ceil()))
}

// Reduce una serie larga a min/max por columna, conservando la forma.
fn diezmar(xs: &[f64], ys: &[f64], columnas: usize) -> Vec<(f64, f64)> {
    let n = xs.len();
    if n <= columnas * 2 {
        return xs.iter().copied().zip(ys.iter().copied()).collect();
    }

    let mut puntos = Vec::with_capacity(columnas * 2);
    let paso = n as f64 / columnas as f64;

    for c in 0..columnas {
        let a = (c as f64 * paso) as usize;
        let b = (((c + 1) as f64 * paso) as usize).min(n);
        if a >= b {
            continue;
        }
        let (mut i_min, mut i_max) = (a, a);
        for i in a..b {
            if ys[i] < ys[i_min] { i_min = i; }
            if ys[i] > ys[i_max] { i_max = i; }
        }
        let (p, q) = if i_min < i_max { (i_min, i_max) } else { (i_max, i_min) };
        puntos.push((xs[p], ys[p]));
        if q != p {
            puntos.push((xs[q], ys[q]));
        }
    }

    puntos
}


// ============================================================
// GRÁFICAS
// ============================================================

fn traza(ruta: &Path, nombre: &str, sweeps: &[usize], valores: &[f64]) -> Resultado<()> {
    let raiz = lienzo(ruta);
    raiz.fill(&FONDO)?;

    let xs: Vec<f64> = sweeps.iter().map(|&s| s as f64).collect();
    let (x0, x1) = extension(xs.iter().copied());
    let (y0, y1) = extension(valores.iter().copied());
    let media = promedio(valores);

    let mut chart = ChartBuilder::on(&raiz)
        .caption(format!("{}  (media = {:.5})", nombre, media), (FUENTE, 20).into_font().color(&TINTA))
        .margin(16)
        .x_label_area_size(40)
        .y_label_area_size(64)
        .build_cartesian_2d(x0..x1, y0..y1)?;

    malla!(chart, "sweep", nombre);

    chart.draw_series(DashedLineSeries::new(vec![(x0, media), (x1, media)], 4, 3, TENUE.stroke_width(1)))?;
    chart.draw_series(LineSeries::new(diezmar(&xs, valores, 1200), SERIE_1.stroke_width(1)))?;

    raiz.present()?;
    Ok(())
}

fn histograma(ruta: &Path, nombre: &str, valores: &[f64], bins: usize) -> Resultado<()> {
    let raiz = lienzo(ruta);
    raiz.fill(&FONDO)?;

    let (a, b) = extension(valores.iter().copied());
    let ancho = (b - a) / bins as f64;
    let mut cuentas = vec![0usize; bins];
    for &v in valores {
        let i = (((v - a) / ancho) as usize).min(bins - 1);
        cuentas[i] += 1;
    }
    let densidad: Vec<f64> = cuentas.iter().map(|&c| c as f64 / valores.len() as f64 / ancho).collect();
    let maximo = densidad.iter().cloned().fold(0.0, f64::max) * 1.05;
    let media = promedio(valores);

    let mut chart = ChartBuilder::on(&raiz)
        .caption(format!("Histograma de {}", nombre), (FUENTE, 20).into_font().color(&TINTA))
        .margin(16)
        .x_label_area_size(40)
        .y_label_area_size(64)
        .build_cartesian_2d(a..b, 0.0..maximo.max(1e-12))?;

    malla!(chart, nombre, "densidad");

    // 2 px de separación entre barras, en unidades de datos.
    let hueco = (b - a) / 800.0;
    chart.draw_series(densidad.iter().enumerate().map(|(i, &d)| {
        let x0 = a + i as f64 * ancho + hueco;
        let x1 = a + (i + 1) as f64 * ancho - hueco;
        Rectangle::new([(x0, 0.0), (x1, d)], SERIE_1.filled())
    }))?;

    chart.draw_series(DashedLineSeries::new(vec![(media, 0.0), (media, maximo)], 4, 3, TENUE.stroke_width(1)))?
        .label(format!("media = {:.5}", media))
        .legend(clave_referencia());

    leyenda!(chart);
    raiz.present()?;
    Ok(())
}

fn grafica_autocorrelacion(ruta: &Path, nombre: &str, valores: &[f64], cada: usize) -> Resultado<()> {
    let tau = autocorrelacion_integrada(valores);
    let max_lag = ((6.0 * tau).ceil() as usize).max(20);
    let rho = autocorrelacion(valores, max_lag);

    let raiz = lienzo(ruta);
    raiz.fill(&FONDO)?;

    let x1 = ((rho.len() - 1) * cada) as f64;
    let y0 = rho.iter().cloned().fold(0.0, f64::min);

    let mut chart = ChartBuilder::on(&raiz)
        .caption(
            format!("Autocorrelación de {}  (τ_int = {:.1} sweeps)", nombre, tau * cada as f64),
            (FUENTE, 20).into_font().color(&TINTA),
        )
        .margin(16)
        .x_label_area_size(40)
        .y_label_area_size(64)
        .build_cartesian_2d(0.0..x1.max(1.0), (y0 - 0.05)..1.05)?;

    malla!(chart, "retraso (sweeps)", "ρ(t)");

    let t_cada = |t: usize| (t * cada) as f64;

    chart.draw_series(DashedLineSeries::new(
        (0..rho.len()).map(|t| (t_cada(t), (-(t as f64) / tau).exp())),
        4, 3, TENUE.stroke_width(1),
    ))?
        .label("exp(−t/τ_int)")
        .legend(clave_referencia());

    chart.draw_series(DashedLineSeries::new(
        vec![(tau * cada as f64, y0 - 0.05), (tau * cada as f64, 1.05)],
        4, 3, TENUE.stroke_width(1),
    ))?;

    chart.draw_series(LineSeries::new(
        rho.iter().enumerate().map(|(t, &r)| (t_cada(t), r)),
        SERIE_1.stroke_width(2),
    ))?
        .label("ρ(t)")
        .legend(clave_linea(SERIE_1));

    leyenda!(chart);
    raiz.present()?;
    Ok(())
}

fn grafica_correlaciones(ruta: &Path, correlaciones: &[CorrelacionMedia]) -> Resultado<()> {
    let raiz = lienzo(ruta);
    raiz.fill(&FONDO)?;

    let r_max = correlaciones.iter().map(|c| c.r).max().unwrap_or(1).max(2) as f64;
    let (y0, y1) = extension_log(correlaciones.iter().flat_map(|c| [c.c, c.cc]));

    let mut chart = ChartBuilder::on(&raiz)
        .caption("Correlación espacial", (FUENTE, 20).into_font().color(&TINTA))
        .margin(16)
        .x_label_area_size(40)
        .y_label_area_size(64)
        .build_cartesian_2d((1.0..r_max).log_scale(), (y0..y1).log_scale())?;

    malla!(chart, "r (distancia en la red)", "C(r)");

    if let Some(ancla) = correlaciones.iter().find(|c| c.cc > 0.0) {
        chart.draw_series(DashedLineSeries::new(
            correlaciones.iter().map(|c| (c.r as f64, ancla.cc * (c.r as f64 / ancla.r as f64).powf(-0.25))),
            4, 3, TENUE.stroke_width(1),
        ))?
            .label("r^(−1/4)  (η = 1/4, T_c)")
            .legend(clave_referencia());
    }

    let positivos = |f: fn(&CorrelacionMedia) -> f64| -> Vec<(f64, f64)> {
        correlaciones.iter().filter(|c| f(c) > 0.0).map(|c| (c.r as f64, f(c))).collect()
    };

    chart.draw_series(LineSeries::new(positivos(|c| c.c), SERIE_1.stroke_width(2)))?
        .label("C(r)")
        .legend(clave_linea(SERIE_1));
    chart.draw_series(LineSeries::new(positivos(|c| c.cc), SERIE_2.stroke_width(2)))?
        .label("Cc(r) = C(r) − ⟨m⟩²")
        .legend(clave_linea(SERIE_2));

    leyenda!(chart);
    raiz.present()?;
    Ok(())
}

// n(s) por sitio y por snapshot, con bins logarítmicos; excluye el mayor
// cluster de cada snapshot. Devuelve (centro del bin, n(s)).
pub fn distribucion_log(clusters: &ClustersLeidos, n: usize) -> Vec<(f64, f64)> {
    let snapshots = &clusters.snapshots;
    if snapshots.is_empty() {
        return Vec::new();
    }

    let maximo = snapshots.iter().filter_map(|(_, t)| t.first().copied()).max().unwrap_or(1);
    let mut bordes = vec![1usize];
    while *bordes.last().unwrap() <= maximo {
        let ultimo = *bordes.last().unwrap();
        bordes.push((ultimo + 1).max((ultimo as f64 * 1.35).ceil() as usize));
    }

    let mut cuentas = vec![0usize; bordes.len() - 1];
    for (_, tamanios) in snapshots {
        for &t in tamanios.iter().skip(1) {
            let b = bordes.partition_point(|&x| x <= t) - 1;
            if b < cuentas.len() {
                cuentas[b] += 1;
            }
        }
    }

    cuentas.iter().enumerate()
        .filter(|(_, c)| **c > 0)
        .map(|(b, &c)| {
            let ancho = (bordes[b + 1] - bordes[b]) as f64;
            let centro = (bordes[b] as f64 * (bordes[b + 1] - 1) as f64).sqrt();
            (centro, c as f64 / ancho / snapshots.len() as f64 / n as f64)
        })
        .collect()
}

fn distribucion_clusters(ruta: &Path, clusters: &[(&str, &ClustersLeidos, RGBColor, f64)], n: usize) -> Resultado<()> {
    let dists: Vec<(&str, Vec<(f64, f64)>, RGBColor, f64)> = clusters.iter()
        .map(|(nombre, c, color, tau)| (*nombre, distribucion_log(c, n), *color, *tau))
        .filter(|(_, d, _, _)| !d.is_empty())
        .collect();

    let raiz = lienzo(ruta);
    raiz.fill(&FONDO)?;

    let (x0, x1) = extension_log(dists.iter().flat_map(|(_, d, _, _)| d.iter().map(|p| p.0)));
    let (y0, y1) = extension_log(dists.iter().flat_map(|(_, d, _, _)| d.iter().map(|p| p.1)));

    let mut chart = ChartBuilder::on(&raiz)
        .caption("Distribución de tamaños de cluster (sin el mayor)", (FUENTE, 20).into_font().color(&TINTA))
        .margin(16)
        .x_label_area_size(40)
        .y_label_area_size(64)
        .build_cartesian_2d((x0..x1).log_scale(), (y0..y1).log_scale())?;

    malla!(chart, "s", "n(s)");

    for (nombre, d, _, tau) in &dists {
        // Referencia de Fisher desplazada (×4) por encima de los datos.
        let ancla = d.iter().find(|p| p.0 >= 2.0).unwrap_or(&d[0]);
        let linea: Vec<(f64, f64)> = (0..=40)
            .map(|k| x0 * (x1 / x0).powf(k as f64 / 40.0))
            .map(|s| (s, 4.0 * ancla.1 * (s / ancla.0).powf(-tau)))
            .filter(|(_, v)| *v >= y0 && *v <= y1)
            .collect();
        let etiqueta = if *nombre == "FK" { "31/15" } else { "379/187" };
        chart.draw_series(DashedLineSeries::new(linea, 4, 3, TENUE.stroke_width(1)))?
            .label(format!("s^(−τ), τ = {} ({})", etiqueta, nombre))
            .legend(clave_referencia());
    }

    for (nombre, d, color, _) in &dists {
        let color = *color;
        chart.draw_series(LineSeries::new(d.iter().copied(), color.stroke_width(2)))?
            .label(*nombre)
            .legend(clave_linea(color));
        chart.draw_series(d.iter().map(|&p| Circle::new(p, 3, color.filled())))?;
    }

    leyenda!(chart);
    raiz.present()?;
    Ok(())
}

fn serie_clusters(
    ruta: &Path,
    titulo: &str,
    eje_y: &str,
    clusters: &[(&str, &ClustersLeidos, RGBColor, f64)],
    valor: impl Fn(&[usize]) -> f64,
) -> Resultado<()> {
    let series: Vec<(&str, Vec<(f64, f64)>, RGBColor)> = clusters.iter()
        .map(|(nombre, c, color, _)| {
            let puntos = c.snapshots.iter().enumerate()
                .map(|(i, (sweep, t))| (sweep.unwrap_or(i) as f64, valor(t)))
                .collect();
            (*nombre, puntos, *color)
        })
        .collect();

    let usa_sweep = clusters.iter().all(|(_, c, _, _)| c.snapshots.iter().all(|(s, _)| s.is_some()));

    let raiz = lienzo(ruta);
    raiz.fill(&FONDO)?;

    let (x0, x1) = extension(series.iter().flat_map(|(_, p, _)| p.iter().map(|q| q.0)));
    let (y0, y1) = extension(series.iter().flat_map(|(_, p, _)| p.iter().map(|q| q.1)));

    let mut chart = ChartBuilder::on(&raiz)
        .caption(titulo, (FUENTE, 20).into_font().color(&TINTA))
        .margin(16)
        .x_label_area_size(40)
        .y_label_area_size(64)
        .build_cartesian_2d(x0..x1, y0..y1)?;

    malla!(chart, if usa_sweep { "sweep" } else { "snapshot" }, eje_y);

    for (nombre, puntos, color) in &series {
        let color = *color;
        let ys: Vec<f64> = puntos.iter().map(|p| p.1).collect();
        let media = promedio(&ys);
        let ancho = if puntos.len() > 400 { 1 } else { 2 };
        chart.draw_series(LineSeries::new(puntos.iter().copied(), color.stroke_width(ancho)))?
            .label(format!("{} (media = {:.4})", nombre, media))
            .legend(clave_linea(color));
    }

    leyenda!(chart);
    raiz.present()?;
    Ok(())
}
