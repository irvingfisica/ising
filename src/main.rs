mod sistema;
mod simulacion_sqgrids;

use std::io::{Write};
use std::fs::File;
use std::error::Error;
use simulacion_sqgrids::{construir_temps, construir_temps_propors};
use simulacion_sqgrids::{simular_temperatura, simular_temperatura_proporcion};
use simulacion_sqgrids::{condiciones,crear_carpeta_ejecucion,crear_sistema_base};
use sistema::{Sistema,Dinamica, Inicial};

fn main() -> Result<(), Box<dyn Error>> {

    ensamble_grid_meta()?;

    Ok(())
}

#[allow(dead_code)]
fn simular_grid() -> Result<(), Box<dyn Error>> {
    let mut rng = rand::rng();
    let temp = 2.269185;
    //temp = 4.0;
    let mut sistem = Sistema::square_grid(100, 1.0, 0.0, temp, Inicial::Random,&mut rng);

    let mut mapa = File::create("mapa_tc.txt")?;
    let mut red = File::create("red_tc.txt")?;
    let mut fotos = File::create("fotografias_tc.txt")?;
    let mut magnetizaciones = File::create("magnetizacion_tc.csv")?;

    sistem.escribir_mapa(&mut mapa)?;
    sistem.escribir_red(&mut red)?;

    writeln!(fotos,"{}",sistem.fotografia())?;
    writeln!(magnetizaciones,"t,M")?;
    writeln!(magnetizaciones,"0,{}",sistem.magnetizacion())?;
    
    for it in 1..10000 {
        sistem.sweep(&mut rng, &Dinamica::Glauber)?;
        writeln!(fotos,"{}",sistem.fotografia())?;
        writeln!(magnetizaciones,"{},{}",it,sistem.magnetizacion())?;
    }

    Ok(())
}

#[allow(dead_code)]
fn ensamble_grid() -> Result<(), Box<dyn Error>> {
    let temperaturas = construir_temps();

    let carpeta = crear_carpeta_ejecucion().map_err(|e| format!("{e}"))?;

    crear_sistema_base(&carpeta).map_err(|e| format!("{e}"))?;

    println!(
        "Resultados: {}",
        carpeta.display()
    );

    condiciones(temperaturas.len());

    for temperatura in temperaturas {

        println!(
            "Simulando T = {:.6}",
            temperatura
        );

        simular_temperatura(temperatura, &carpeta).map_err(|e| format!("{e}"))?;
    };

    Ok(())
}

fn ensamble_grid_meta() -> Result<(), Box<dyn Error>> {
    let tuplas = construir_temps_propors();

    let carpeta = crear_carpeta_ejecucion().map_err(|e| format!("{e}"))?;

    crear_sistema_base(&carpeta).map_err(|e| format!("{e}"))?;

    println!(
        "Resultados: {}",
        carpeta.display()
    );

    condiciones(tuplas.len());

    for (temperatura,proporcion) in tuplas {
        println!(
            "Simulando T = {:.6}, P = {:.2}",
            temperatura,
            proporcion
        );

        simular_temperatura_proporcion(temperatura, proporcion, &carpeta).map_err(|e| format!("{e}"))?;
    }

    Ok(())
}


