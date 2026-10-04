use std::error::Error;
use std::fmt::Write as FmtWrite;
use std::fs;
use std::path::{Path, PathBuf};

// Visor web (D3) de los archivos que escribe `Resultado::escribir`.
const VISOR_HTML: &str = include_str!("../visor/visor.html");

// Copia `visor.html` en `directorio` y genera `datos.js` con el contenido de
// todos los CSV de ese directorio. Así el visor se abre con doble clic, sin
// servidor, y funciona también con corridas viejas.
//
// Devuelve la ruta del visor.
pub fn instalar<P: AsRef<Path>>(directorio: P) -> Result<PathBuf, Box<dyn Error>> {
    let dir = directorio.as_ref();

    let mut nombres: Vec<String> = Vec::new();
    for entrada in fs::read_dir(dir)? {
        let ruta = entrada?.path();
        if ruta.extension().is_some_and(|e| e == "csv") {
            if let Some(nombre) = ruta.file_name().and_then(|n| n.to_str()) {
                nombres.push(nombre.to_string());
            }
        }
    }
    nombres.sort();

    if nombres.is_empty() {
        return Err(format!("No hay archivos CSV en {}", dir.display()).into());
    }

    let mut js = String::from("window.DATOS_ISING = {\n");
    for nombre in &nombres {
        let contenido = fs::read_to_string(dir.join(nombre))?;
        writeln!(js, "  {}: {},", cadena_json(nombre), cadena_json(&contenido))?;
    }
    js.push_str("};\n");

    fs::write(dir.join("datos.js"), js)?;

    let ruta = dir.join("visor.html");
    fs::write(&ruta, VISOR_HTML)?;

    Ok(ruta)
}

// Solo escribe `visor.html` (para cargar los CSV a mano desde el navegador).
pub fn escribir_html<P: AsRef<Path>>(ruta: P) -> Result<(), Box<dyn Error>> {
    fs::write(ruta, VISOR_HTML)?;
    Ok(())
}

fn cadena_json(texto: &str) -> String {
    let mut s = String::with_capacity(texto.len() + 2);
    s.push('"');
    for c in texto.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            '\t' => s.push_str("\\t"),
            '\u{2028}' => s.push_str("\\u2028"),
            '\u{2029}' => s.push_str("\\u2029"),
            c if (c as u32) < 0x20 => { let _ = write!(s, "\\u{:04x}", c as u32); },
            c => s.push(c),
        }
    }
    s.push('"');
    s
}
