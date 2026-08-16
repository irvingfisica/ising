use std::collections::HashMap;
use std::error::Error;
use rand::{Rng, RngExt};
use rand::distr::{Distribution,StandardUniform};
use std::fmt::Write as FmtWrite;
use std::io::{self, Write};
use std::fs::File;

fn main() -> Result<(), Box<dyn Error>> {

    let mut rng = rand::rng();
    let mut sistem = Sistema::square_grid(10, 1.0, 0.0, 0.1);

    let mut mapa = File::create("mapa.txt")?;
    let mut red = File::create("red.txt")?;
    let mut fotos = File::create("fotografias.txt")?;
    let mut magnetizaciones = File::create("magnetizacion.txt")?;

    sistem.escribir_mapa(&mut mapa)?;
    sistem.escribir_red(&mut red)?;

    writeln!(fotos,"{}",sistem.fotografia())?;
    writeln!(magnetizaciones,"0,{}",sistem.magnetizacion())?;
    
    for it in 1..1000 {
        sistem.sweep(&mut rng, Dinamica::Glauber)?;
        writeln!(fotos,"{}",sistem.fotografia())?;
        writeln!(magnetizaciones,"{},{}",it,sistem.magnetizacion())?;
    }

    Ok(())
}

pub struct Sistema {
    mapa: HashMap<String,usize>,
    elementos: Vec<Celda>,
    j: f64,
    h: f64,
    temp: f64
}

impl Sistema {

    pub fn square_grid(n: usize, j: f64, h: f64, temp: f64) -> Self {
        let mut sistema = Sistema {
            mapa: HashMap::new(),
            elementos: Vec::new(),
            j,h,temp
        };

        let mut rnd = rand::rng();

        let mut cta = 0;
        for i_idx in 0..n {
            for j_idx in 0..n {
                let id:String = format!("{i_idx}-{j_idx}");
                let estado: Estado = rnd.random(); 
                let mut celda = Celda::new(&id, estado);

                let up = if j_idx == 0 {
                    n - 1
                } else {
                    j_idx - 1
                };
                let down = if j_idx == n - 1 {
                    0
                } else {
                    j_idx + 1
                };
                let left = if i_idx == 0 {
                    n - 1
                } else {
                    i_idx - 1
                };
                let right = if i_idx == n - 1 {
                    0
                } else {
                    i_idx + 1
                };

                let v1 = format!("{i_idx}-{up}");
                let v2 = format!("{i_idx}-{down}");
                let v3 = format!("{left}-{j_idx}");
                let v4 = format!("{right}-{j_idx}");
                celda.add_vecino(v1);
                celda.add_vecino(v2);
                celda.add_vecino(v3);
                celda.add_vecino(v4);

                sistema.mapa.insert(id, cta);
                sistema.elementos.push(celda);
                cta = cta + 1;
            }
        }

        for ele in sistema.elementos.iter_mut() {
            ele.seguir_vecinos(&sistema.mapa);
        }

        sistema
    }

    pub fn campo_local(&self,celda: &Celda) -> f64 {
        let mut suma = 0.0;
        for pos in celda.veclist.iter() {

            if let Some(vcelda) = self.elementos.get(*pos) {
                suma = suma + vcelda.spin();
            }
        }

        suma*self.j + self.h
    }

    pub fn magnetizacion(&self) -> f64 {
        let mut suma = 0.0;
        for celda in self.elementos.iter() {
            suma = suma + celda.spin()
        }

        suma / self.elementos.len() as f64
    }

    pub fn fotografia(&self) -> String {
        let mut eactivo = Estado::Positivo;
        let mut foto = String::new();
        for (pos,celda) in self.elementos.iter().enumerate() {
            if celda.estado != eactivo {
                write!(&mut foto, "{} ", pos).unwrap();
                eactivo.flip();
            }
        }

        foto
    }

    pub fn glauber<R: Rng>(&mut self, pos: usize, rng: &mut R) -> Result<(), Box<dyn Error>> {
        let beta = 1.0/self.temp;

        let campo = {
            match self.elementos.get(pos) {
                Some(celda) => {
                    self.campo_local(celda)
                },
                None => return Err("No existe la celda".into())
            }
        };

        let celda = &mut self.elementos[pos];
        
        let expfactor = (-2.0*beta*campo).exp();
        let pplus = 1.0/(1.0 + expfactor);

        let randp: f64 = rng.random();
        if randp < pplus {
            celda.set_state(Estado::Positivo);
        } else {
            celda.set_state(Estado::Negativo);
        }

        Ok(())
    }

    pub fn metropolis<R: Rng>(&mut self, pos: usize, rng: &mut R) -> Result<(), Box<dyn Error>> {
        let beta = 1.0/self.temp;

        let campo = {
            match self.elementos.get(pos) {
                Some(celda) => {
                    self.campo_local(celda)
                },
                None => return Err("No existe la celda".into())
            }
        };

        let celda = &mut self.elementos[pos];

        let delta = 2.0*campo*celda.spin();
        let expfactor = (-1.0*beta*delta).exp();
        let pacc = expfactor.min(1.0);

        let randp: f64 = rng.random();
        if randp < pacc {
            celda.flip();
        };

        Ok(()) 

    }

    pub fn sweep<R: Rng>(&mut self, rng: &mut R, dinamica: Dinamica) -> Result<(), Box<dyn Error>> {
        for _ in 0..self.elementos.len() {
            let pos = rng.random_range(0..self.elementos.len());
            match dinamica {
                Dinamica::Glauber => self.glauber(pos, rng)?,
                Dinamica::Metropolis => self.metropolis(pos, rng)?,
            }
        };

        Ok(())
    }

    pub fn escribir_mapa<W: Write>(&self, archivo: &mut W) -> io::Result<()> {
        for (pos,celda) in self.elementos.iter().enumerate() {
            writeln!(archivo, "{} {}", pos, celda.id)?;
        };

        Ok(())
    }

    pub fn escribir_red<W: Write>(&self, archivo: &mut W) -> io::Result<()> {
        for (pos, celda) in self.elementos.iter().enumerate() {
            write!(archivo, "{}", pos)?;

            for vecino in &celda.veclist {
                write!(archivo, " {}", vecino)?;
            }

            writeln!(archivo)?;
        };

        Ok(())
    }
}

pub enum Dinamica {
    Glauber,
    Metropolis
}

pub struct Celda {
    id: String,
    vecinos: Vec<String>,
    veclist: Vec<usize>,
    estado: Estado
}

impl Celda {
    pub fn new(id: &str, estado: Estado) -> Self {
        Celda {
            id: id.to_string(),
            vecinos: Vec::new(),
            veclist: Vec::new(),
            estado: estado
        }
    }

    pub fn add_vecino(&mut self,idv: String) {
        self.vecinos.push(idv);
    }

    pub fn seguir_vecinos(&mut self, mapa: &HashMap<String,usize>) {
        for id in self.vecinos.iter() {
            let posicion = mapa.get(id);

            if let Some(pos) = posicion {
                self.veclist.push(*pos);
            }
        }
    }

    pub fn set_state(&mut self,estado: Estado) {
        self.estado = estado;
    }

    pub fn flip(&mut self) {
        self.estado.flip();
    }

    pub fn spin(&self) -> f64 {
        self.estado.spin()
    }
}

#[derive(PartialEq)]
pub enum Estado {
    Positivo,
    Negativo
}

impl Estado {
    pub fn spin(&self) -> f64 {
        match self {
            Estado::Positivo => 1.0,
            Estado::Negativo => -1.0
        }
    }

    pub fn flip(&mut self) {
        *self = match self {
            Estado::Negativo => Estado::Positivo,
            Estado::Positivo => Estado::Negativo
        }
    }
}

impl Distribution<Estado> for StandardUniform {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Estado {
        let index = rng.random_range(0..2);
        match index {
            0 => Estado::Positivo,
            1 => Estado::Negativo,
            _ => unreachable!()
        }
    }
}