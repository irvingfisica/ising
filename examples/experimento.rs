use std::error::Error;

use rand::SeedableRng;
use rand::rngs::StdRng;

use ising::sistema::{Sistema, Inicial, Dinamica};
use ising::simulaciones::{Experimento, PlanMedicion, Metrica};
use ising::visor;

const L: usize = 32;

const J: f64 = 1.0;
const B: f64 = 0.0;
const TEMP: f64 = 2.269185314;

const SEED: u64 = 123456789;


fn main() -> Result<(), Box<dyn Error>> {

    let mut rng = StdRng::seed_from_u64(SEED);

    let mut sistema = Sistema::square_grid(L, J, B, TEMP, Inicial::Random, &mut rng);

    let experimento = Experimento::new(5_000, 10_000, Dinamica::Glauber)
        .medir(PlanMedicion::cada(1, vec![
            Metrica::Magnetizacion,
            Metrica::MagnetizacionAbsoluta,
            Metrica::Energia,
            Metrica::C1,
        ]))
        .medir(PlanMedicion::cada(100, vec![
            Metrica::Correlaciones { r_max: 16 },
            Metrica::ClustersGeometricos,
            Metrica::ClustersFK,
        ]))
        .progreso(1000);

    let resultado = experimento.simular(&mut sistema, &mut rng)?;

    println!("{}", resultado);

    resultado.escribir("datos")?;

    let ruta = visor::instalar("datos")?;
    println!("Visor: {}", ruta.display());

    Ok(())
}
