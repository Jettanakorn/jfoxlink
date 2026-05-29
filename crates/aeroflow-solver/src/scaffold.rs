use aeroflow_core::types::{CouplingStrategy, PhysicsModule, SolverDesign, SolverTemplate, TimeTreatment};

/// Generates the directory scaffold for a custom OpenFOAM solver
pub struct SolverScaffold;

impl SolverScaffold {
    /// Return the base template solver name for a SolverTemplate
    pub fn base_solver_name(tmpl: &SolverTemplate) -> &'static str {
        match tmpl {
            SolverTemplate::MhdSimpleFoam => "simpleFoam",
            SolverTemplate::MhdReactingFoam => "reactingFoam",
            SolverTemplate::PlasmaActuatorFoam => "simpleFoam",
            SolverTemplate::HyperReactingFoam => "rhoCentralFoam",
            SolverTemplate::ChtRotatingFoam => "chtMultiRegionSimpleFoam",
            SolverTemplate::ViscoelasticHeatFoam => "simpleFoam",
            SolverTemplate::BubblyReactingFoam => "reactingTwoPhaseEulerFoam",
            SolverTemplate::AblationFoam => "rhoCentralFoam",
            SolverTemplate::DsmcReactingFoam => "dsmcFoam",
            SolverTemplate::MagneticConvectionFoam => "buoyantSimpleFoam",
            SolverTemplate::RotorAeroFoam => "pimpleFoam",
            SolverTemplate::CoupledPlasmaFoam => "",
            SolverTemplate::PemfcFoam => "simpleFoam",
            SolverTemplate::PemfcThermalFoam => "buoyantSimpleFoam",
            SolverTemplate::PemfcTwoPhaseFoam => "reactingTwoPhaseEulerFoam",
            SolverTemplate::Custom => "",
        }
    }

    /// Generate Make/files content
    pub fn generate_make_files(solver_name: &str) -> String {
        format!(
            r#"{}.C

EXE = $(FOAM_USER_APPBIN)/{}
"#, solver_name, solver_name)
    }

    /// Generate Make/options content based on enabled modules
    pub fn generate_make_options(design: &SolverDesign) -> String {
        let mut inc_lines = vec![
            "    -I$(LIB_SRC)/finiteVolume/lnInclude",
            "    -I$(LIB_SRC)/meshTools/lnInclude",
            "    -I$(LIB_SRC)/sampling/lnInclude",
            "    -I$(LIB_SRC)/TurbulenceModels/turbulenceModels/lnInclude",
        ];
        let mut lib_lines = vec![
            "    -lfiniteVolume",
            "    -lmeshTools",
            "    -lturbulenceModels",
            "    -lsampling",
        ];

        for m in &design.modules {
            match m {
                PhysicsModule::Compressible | PhysicsModule::HeatTransfer => {
                    inc_lines.push("    -I$(LIB_SRC)/thermophysicalModels/basic/lnInclude");
                    inc_lines.push("    -I$(LIB_SRC)/thermophysicalModels/fluidThermo/lnInclude");
                    lib_lines.push("    -lfluidThermophysicalModels");
                }
                PhysicsModule::SpeciesTransport | PhysicsModule::ChemicalReactions => {
                    inc_lines.push("    -I$(LIB_SRC)/thermophysicalModels/reactionThermo/lnInclude");
                    inc_lines.push("    -I$(LIB_SRC)/combustionModels/lnInclude");
                    lib_lines.push("    -lreactionThermophysicalModels");
                    lib_lines.push("    -lcombustionModels");
                }
                PhysicsModule::Turbulence => {
                    inc_lines.push("    -I$(LIB_SRC)/TurbulenceModels/compressible/lnInclude");
                    lib_lines.push("    -lcompressibleTurbulenceModels");
                }
                PhysicsModule::TwoPhase => {
                    inc_lines.push("    -I$(LIB_SRC)/multiphase/lnInclude");
                    lib_lines.push("    -lmultiphase");
                }
                PhysicsModule::Radiation => {
                    inc_lines.push("    -I$(LIB_SRC)/thermophysicalModels/radiation/lnInclude");
                }
                PhysicsModule::RotatingFrame => {
                    inc_lines.push("    -I$(LIB_SRC)/fvOptions/lnInclude");
                    lib_lines.push("    -lfvOptions");
                }
                PhysicsModule::PorousMedia => {
                    inc_lines.push("    -I$(LIB_SRC)/fvOptions/lnInclude");
                    lib_lines.push("    -lfvOptions");
                }
                PhysicsModule::ParticleTracking => {
                    inc_lines.push("    -I$(LIB_SRC)/lagrangian/basic/lnInclude");
                    lib_lines.push("    -llagrangian");
                }
                PhysicsModule::CustomEOS | PhysicsModule::CustomViscosity => {
                    inc_lines.push("    -I$(LIB_SRC)/transportModels/lnInclude");
                    lib_lines.push("    -ltransportModels");
                }
                _ => {}
            }
        }

        inc_lines.sort();
        inc_lines.dedup();
        lib_lines.sort();
        lib_lines.dedup();

        format!(
            r#"EXE_INC = \
{}

EXE_LIBS = \
{}
"#, inc_lines.join(" \\\n"), lib_lines.join(" \\\n"))
    }

    /// Generate solver description comment header
    fn description_header(design: &SolverDesign) -> String {
        format!(
            r#"/*---------------------------------------------------------------------------*\
  =========                 |
  \\      /  F ield         | OpenFOAM: The Open Source CFD Toolbox
   \\    /   O peration     |
    \\  /    A nd           | Custom solver generated by AeroFlow Agent
     \\/     M anipulation  |
-------------------------------------------------------------------------------
Description: {}
Solver: {}
Physics: {}
\*---------------------------------------------------------------------------*/
"#,
            design.description,
            design.solver_name,
            design.modules.iter().map(|m| format!("{:?}", m)).collect::<Vec<_>>().join(", "),
        )
    }

    /// Include headers based on enabled modules
    fn solver_includes(design: &SolverDesign) -> String {
        let mut includes = String::from("#include \"fvCFD.H\"\n");
        let has_compressible = design.modules.contains(&PhysicsModule::Compressible);
        let has_thermal = design.modules.contains(&PhysicsModule::HeatTransfer);
        let has_reactions = design.modules.contains(&PhysicsModule::ChemicalReactions);
        let has_species = design.modules.contains(&PhysicsModule::SpeciesTransport);
        let has_em = design.modules.contains(&PhysicsModule::Electromagnetic);
        let has_radiation = design.modules.contains(&PhysicsModule::Radiation);
        let has_turb = design.modules.contains(&PhysicsModule::Turbulence);
        let has_porous = design.modules.contains(&PhysicsModule::PorousMedia);

        if has_compressible || has_thermal {
            includes.push_str("#include \"psiThermo.H\"\n");
            includes.push_str("#include \"turbulentFluidThermoModel.H\"\n");
        }
        if has_reactions {
            includes.push_str("#include \"combustionModel.H\"\n");
        }
        if has_radiation {
            includes.push_str("#include \"radiation/radiationModel.H\"\n");
        }
        if has_em {
            includes.push_str("#include \"electromagneticModel.H\"\n");
        }
        if has_species {
            includes.push_str("#include \"fvOptions.H\"\n");
        }
        if has_porous {
            includes.push_str("#include \"fvOptions.H\"\n");
        }
        if has_turb && !has_compressible {
            includes.push_str("#include \"turbulentTransportModel.H\"\n");
        }
        includes
    }

    /// Generate the momentum equation (UEqn.H)
    pub fn generate_ueqn(design: &SolverDesign) -> String {
        let has_em = design.modules.contains(&PhysicsModule::Electromagnetic);
        let has_porous = design.modules.contains(&PhysicsModule::PorousMedia);
        let has_particles = design.modules.contains(&PhysicsModule::ParticleTracking);
        let ddt = match design.time_treatment {
            TimeTreatment::Steady => String::new(),
            TimeTreatment::UnsteadyFirstOrder => "    fvm::ddt(rho, U)\n  + ".to_string(),
            TimeTreatment::UnsteadySecondOrder => "    fvm::ddt(rho, U)\n  + ".to_string(),
        };

        let mut ueqn = format!(
            r#"// Momentum equation
tmp<fvVectorMatrix> tUEqn
(
{}
    fvm::div(phi, U)
  + MRF.DDt(phi, U)
  + turbulence->divDevReff(U)
 ==
    fvOptions(U)
"#,
            ddt,
        );

        if has_em {
            ueqn.push_str("  + F_EM\n");
        }
        if has_particles {
            ueqn.push_str("  + F_p\n");
        }

        ueqn.push_str(
            r#");
fvVectorMatrix& UEqn = tUEqn.ref();
UEqn.relax();

fvOptions.constrain(UEqn);

if (simple.momentumPredictor())
{
    solve(UEqn == -fvc::grad(p) + fvOptions(U));
}
"#);
        if has_porous {
            ueqn.push_str(
                "\n// Porous media correction\n\
            MRF.correctBoundaryVelocity(U);\n"
            );
        }
        ueqn
    }

    /// Generate the pressure equation (pEqn.H)
    pub fn generate_peqn(design: &SolverDesign) -> String {
        let has_compressible = design.modules.contains(&PhysicsModule::Compressible);
        if has_compressible {
            String::from(
                r#"// Pressure equation (compressible)
volScalarField rAU(1.0 / UEqn.A());
surfaceScalarField phiHbyD
(
    "phiHbyD",
    fvc::flux(HbyA)
  + fvc::interpolate(rAU) * fvc::ddtCorr(U, phi)
);

adjustPhi(phiHbyD, U, p);

// Non-orthogonal pressure corrector loop
while (simple.correctNonOrthogonal())
{
    fvScalarMatrix pEqn
    (
        fvm::laplacian(rAU, p) == fvc::div(phiHbyD)
    );
    pEqn.setReference(pRefCell, pRefValue);
    pEqn.solve();
}

phi = phiHbyD - pEqn.flux();
"#)
        } else {
            String::from(
                r#"// Pressure equation (incompressible)
volScalarField rAU(1.0 / UEqn.A());
volVectorField HbyA(constrainHbyA(rAU * UEqn.H(), U, p));
surfaceScalarField phiHbyD
(
    "phiHbyD",
    fvc::flux(HbyA)
  + fvc::interpolate(rAU) * fvc::ddtCorr(U, phi)
);

adjustPhi(phiHbyD, U, p);

// Non-orthogonal pressure corrector loop
while (simple.correctNonOrthogonal())
{
    fvScalarMatrix pEqn
    (
        fvm::laplacian(rAU, p) == fvc::div(phiHbyD)
    );
    pEqn.setReference(pRefCell, pRefValue);
    pEqn.solve();
}

phi = phiHbyD - pEqn.flux();
"#)
        }
    }

    /// Generate the energy equation (EEqn.H)
    pub fn generate_eeqn(design: &SolverDesign) -> String {
        if !design.modules.contains(&PhysicsModule::HeatTransfer) {
            return String::new();
        }
        let has_reactions = design.modules.contains(&PhysicsModule::ChemicalReactions);
        let has_radiation = design.modules.contains(&PhysicsModule::Radiation);
        let _has_em = design.modules.contains(&PhysicsModule::Electromagnetic);

        let mut src = String::from("    fvOptions(rho, he)");
        if has_reactions {
            src = format!("    Qdot\n  + {}", src);
        }
        if has_radiation {
            src = format!("    rad->Sh(thermo, he)\n  + {}", src);
        }

        format!(
            r#"// Energy equation
tmp<fvScalarMatrix> tEEqn
(
    fvm::ddt(rho, he)
  + fvm::div(phi, he)
  - fvm::laplacian(turbulence->alphaEff(), he)
 ==
    {}
);
fvScalarMatrix& EEqn = tEEqn.ref();
EEqn.relax();
fvOptions.constrain(EEqn);
EEqn.solve();

thermo.correct();
"#, src)
    }

    /// Generate species transport equations (YEqn.H)
    pub fn generate_yeqn(design: &SolverDesign) -> String {
        if !design.modules.contains(&PhysicsModule::SpeciesTransport)
            && !design.modules.contains(&PhysicsModule::ChemicalReactions)
        {
            return String::new();
        }
        let has_reactions = design.modules.contains(&PhysicsModule::ChemicalReactions);

        let combustion_call = if has_reactions {
            "combustion->correct();
volScalarField Qdot(combustion->Qdot());
"
        } else {
            ""
        };

        format!(
            r#"// Species transport equations
{}
forAll(Y, i)
{{
    tmp<fvScalarMatrix> YiEqn
    (
        fvm::ddt(rho, Y[i])
      + fvm::div(phi, Y[i])
      - fvm::laplacian(turbulence->muEff() / Sct, Y[i])
     ==
        fvOptions(rho, Y[i])
    );
    YiEqn->relax();
    fvOptions.constrain(YiEqn.ref());
    YiEqn->solve(mesh.solver("Yi"));
}}
"#, combustion_call)
    }

    /// Generate createFields.H
    pub fn generate_create_fields(design: &SolverDesign) -> String {
        let has_compressible = design.modules.contains(&PhysicsModule::Compressible);
        let has_em = design.modules.contains(&PhysicsModule::Electromagnetic);
        let has_reactions = design.modules.contains(&PhysicsModule::ChemicalReactions);
        let has_species = design.modules.contains(&PhysicsModule::SpeciesTransport);
        let has_particles = design.modules.contains(&PhysicsModule::ParticleTracking);
        let has_radiation = design.modules.contains(&PhysicsModule::Radiation);
        let has_porous = design.modules.contains(&PhysicsModule::PorousMedia);

        let mut fields = String::new();

        if has_compressible {
            fields.push_str(
                r#"// Thermodynamics
autoPtr<psiThermo> pThermo(psiThermo::New(mesh));
psiThermo& thermo = pThermo();
volScalarField& p = thermo.p();
volScalarField& he = thermo.he();
volScalarField& T = thermo.T();

// Density from thermo
volScalarField rho
(
    IOobject("rho", runTime.timeName(), mesh),
    thermo.rho()
);
"#);
        } else {
            fields.push_str(
                r#"// Pressure
volScalarField p
(
    IOobject("p", runTime.timeName(), mesh),
    mesh,
    dimensionedScalar(dimPressure, 0)
);

// Velocity
volVectorField U
(
    IOobject("U", runTime.timeName(), mesh),
    mesh,
    dimensionedVector(dimVelocity, vector::zero)
);

// Density (constant)
volScalarField rho
(
    IOobject("rho", runTime.timeName(), mesh),
    mesh,
    dimensionedScalar(dimDensity, 1.225)
);

#include "createPhi.H"
"#);
        }

        if has_em {
            fields.push_str(
                r#"
// Magnetic field
volVectorField B
(
    IOobject("B", runTime.timeName(), mesh),
    mesh,
    dimensionedVector(dimMagFluxDensity, vector::zero)
);

// Electric potential
volScalarField phiE
(
    IOobject("phiE", runTime.timeName(), mesh),
    mesh,
    dimensionedScalar(dimVoltage, 0)
);
"#);
        }

        if has_reactions || has_species {
            fields.push_str(
                r#"
// Species mass fractions
PtrList<volScalarField> Y(thermo.composition().Y());
"#);
        }

        if has_reactions {
            fields.push_str(
                r#"
// Combustion model
autoPtr<combustionModels::combustionModel> combustion
(
    combustionModels::combustionModel::New(thermo, turbulence())
);
"#);
        }

        if has_radiation {
            fields.push_str(
                r#"
// Radiation model
autoPtr<radiation::radiationModel> rad
(
    radiation::radiationModel::New(T, phi)
);
"#);
        }

        if has_porous {
            fields.push_str(
                r#"
// Porous media zone
#include "createPorousZones.H"
"#);
        }

        if has_particles {
            fields.push_str(
                r#"
// Lagrangian particles
#include "basicKinematicCloud.H"
basicKinematicCloud particles("kinematicCloud", rho, U, g, slgThermo);
"#);
        }

        if !has_compressible {
            // Add turbulence for incompressible
            fields.push_str(
                r#"
// Turbulence model
#include "turbulentTransportModel.H"
"#);
        }

        fields
    }

    /// Generate the main solver .C file
    pub fn generate_main_solver(design: &SolverDesign) -> String {
        let header = Self::description_header(design);
        let includes = Self::solver_includes(design);
        let _fields_header = Self::generate_create_fields(design);

        let _ddt_term = match design.time_treatment {
            TimeTreatment::Steady => String::new(),
            _ => "    while (runTime.run())\n    {\n        runTime++;\n".to_string(),
        };
        let loop_close = match design.time_treatment {
            TimeTreatment::Steady => String::new(),
            _ => "    }\n".to_string(),
        };

        let unsteady_decl = match design.time_treatment {
            TimeTreatment::Steady => String::new(),
            _ => "    #include \"readTimeControls.H\"\n".to_string(),
        };

        let convergence_check = match design.coupling {
            CouplingStrategy::SegregatedSIMPLE => {
                r#"
    // SIMPLE convergence check
    simple.calcResiduals(U, p);
    if (simple.residuals().max() < convergenceCriterion)
    {
        Info << "Solution converged at iteration " << runTime.timeName() << endl;
        break;
    }
"#.to_string()
            }
            _ => String::new(),
        };

        let has_thermal = design.modules.contains(&PhysicsModule::HeatTransfer);
        let has_species = design.modules.contains(&PhysicsModule::SpeciesTransport)
            || design.modules.contains(&PhysicsModule::ChemicalReactions);

        let eeqn_include = if has_thermal { "\n        #include \"EEqn.H\"" } else { "" };
        let yeqn_include = if has_species { "\n        #include \"YEqn.H\"" } else { "" };

        format!(
            r#"{header}
{includes}

// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //

int main(int argc, char *argv[])
{{
    #include "setRootCase.H"
    #include "createTime.H"
    #include "createMesh.H"
    #include "createFields.H"
    #include "createMRF.H"
    #include "createFvOptions.H"
    {unsteady_decl}

    // * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //

    while (simple.loop())
    {{
        Info << "Time = " << runTime.timeName() << endl;

        #include "UEqn.H"
        #include "pEqn.H"
        {eeqn_include}
        {yeqn_include}
        {convergence_check}
        turbulence->correct();
        runTime.write();
    }}

    {loop_close}

    Info << "End\\n" << endl;
    return 0;
}}

// * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * //
"#,
            header = header,
            includes = includes,
            unsteady_decl = unsteady_decl,
            eeqn_include = eeqn_include,
            yeqn_include = yeqn_include,
            convergence_check = convergence_check,
            loop_close = loop_close,
        )
    }

    /// Generate membrane overpotential equation (epsEqn.H) for PEMFC
    pub fn generate_epseqn() -> String {
        String::from(
            r#"// Membrane overpotential / protonic charge conservation
// ∇·(σₘₑₘ ∇ε) + S_ε = 0
// where S_ε = volumetric current source (Butler-Volmer)

volScalarField sigmaMem
(
    IOobject("sigmaMem", runTime.timeName(), mesh),
    mesh,
    dimensionedScalar(dimless / dimResistivity, 10.0)
);

// Overpotential
volScalarField epsilon
(
    IOobject("epsilon", runTime.timeName(), mesh),
    mesh,
    dimensionedScalar(dimVoltage, 0)
);

// Load membrane properties from dictionary
IOdictionary membraneProperties
(
    IOobject
    (
        "membraneProperties",
        runTime.constant(),
        mesh,
        IOobject::MUST_READ,
        IOobject::NO_WRITE
    )
);

// Butler-Volmer current source
volScalarField iBV
(
    IOobject("iBV", runTime.timeName(), mesh),
    mesh,
    dimensionedScalar(dimCurrentDensity, 0)
);

fvScalarMatrix epsEqn
(
    fvm::laplacian(sigmaMem, epsilon)
  + iBV
  ==
    fv::options(epsilon)
);
epsEqn.relax();
epsEqn.solve();
"#)
    }

    /// Generate species transport with Butler-Volmer sources (YEqn.H for PEMFC)
    pub fn generate_pemfc_yeqn() -> String {
        String::from(
            r#"// PEMFC species transport: H₂, O₂, H₂O(g)
// Butler-Volmer source terms applied in catalyst layers

// Hydrogen (anode side)
volScalarField Yh2
(
    IOobject("Yh2", runTime.timeName(), mesh),
    mesh,
    dimensionedScalar(dimless, 0)
);

// Oxygen (cathode side)
volScalarField Yo2
(
    IOobject("Yo2", runTime.timeName(), mesh),
    mesh,
    dimensionedScalar(dimless, 0.23)
);

// Water vapour
volScalarField Yh2o_g
(
    IOobject("Yh2o_g", runTime.timeName(), mesh),
    mesh,
    dimensionedScalar(dimless, 0)
);

// H₂ transport (anode)
fvScalarMatrix Yh2Eqn
(
    fvm::ddt(rho, Yh2)
  + fvm::div(phi, Yh2)
  - fvm::laplacian(turbulence->muEff() / 1.0, Yh2)
 ==
  // Anode source: H₂ → 2H⁺ + 2e⁻
  - (iBV / (2.0 * 96485.0)) * (2.016e-3) // kg/s per m³
  + fvOptions(rho, Yh2)
);
Yh2Eqn.relax();
Yh2Eqn.solve();

// O₂ transport (cathode)
fvScalarMatrix Yo2Eqn
(
    fvm::ddt(rho, Yo2)
  + fvm::div(phi, Yo2)
  - fvm::laplacian(turbulence->muEff() / 1.0, Yo2)
 ==
  // Cathode source: O₂ + 4H⁺ + 4e⁻ → 2H₂O
  - (iBV / (4.0 * 96485.0)) * (31.998e-3) // kg/s per m³
  + fvOptions(rho, Yo2)
);
Yo2Eqn.relax();
Yo2Eqn.solve();

// H₂O(g) transport
fvScalarMatrix Yh2oEqn
(
    fvm::ddt(rho, Yh2o_g)
  + fvm::div(phi, Yh2o_g)
  - fvm::laplacian(turbulence->muEff() / 1.0, Yh2o_g)
 ==
  // Cathode source: water production
  + (iBV / (2.0 * 96485.0)) * (18.015e-3) // kg/s per m³
  + fvOptions(rho, Yh2o_g)
);
Yh2oEqn.relax();
Yh2oEqn.solve();
"#)
    }

    /// Generate energy equation with PEMFC thermal sources (EEqn.H for PemfcThermalFoam)
    pub fn generate_pemfc_eeqn() -> String {
        String::from(
            r#"// PEMFC energy equation with Joule heating and reaction heat
// Joule heating: i² / σₘₑₘ
// Reaction enthalpy: ΔH_rxn * i / (nF)

volScalarField QJoule
(
    IOobject("QJoule", runTime.timeName(), mesh),
    mesh,
    dimensionedScalar(dimPower / dimVolume, 0)
);
QJoule = mag(fvc::grad(epsilon)) * mag(fvc::grad(epsilon)) * sigmaMem;

volScalarField Qrxn
(
    IOobject("Qrxn", runTime.timeName(), mesh),
    mesh,
    dimensionedScalar(dimPower / dimVolume, 0)
);
// Reaction enthalpy ≈ -241.8 kJ/mol (HHV) for H₂ + ½O₂ → H₂O(l)
// Q_rxn = (ΔH / (nF)) * i
Qrxn = (-241.8e3 / (2.0 * 96485.0)) * iBV;

fvScalarMatrix EEqn
(
    fvm::ddt(rho, he)
  + fvm::div(phi, he)
  - fvm::laplacian(turbulence->alphaEff(), he)
 ==
    QJoule + Qrxn
  + fvOptions(rho, he)
);
EEqn.relax();
EEqn.solve();

thermo.correct();
"#)
    }

    /// Generate liquid water saturation equation for two-phase PEMFC
    pub fn generate_saturation_eqn() -> String {
        String::from(
            r#"// Liquid water saturation transport (two-phase model)
// ∂(ερs_l)/∂t + ∇·(ρ_l U_l s_l) = S_w
// where S_w includes water production + condensation/evaporation

volScalarField s_l
(
    IOobject("s_l", runTime.timeName(), mesh),
    mesh,
    dimensionedScalar(dimless, 0)
);

volScalarField S_w
(
    IOobject("S_w", runTime.timeName(), mesh),
    mesh,
    dimensionedScalar(dimless / dimTime, 0)
);

// Condensation/evaporation rate
// k_cond * (λ - λ_sat) * ρ_g * s_g
// Capillary diffusion in GDL

fvScalarMatrix sEqn
(
    fvm::ddt(s_l)
  + fvm::div(phi, s_l)
  - fvm::laplacian(1e-8, s_l) // capillary diffusion coefficient
 ==
    S_w
);
sEqn.relax();
sEqn.solve();

// Update liquid water mass fraction
Yh2o_l = s_l / max(s_l + (1 - s_l) * rho_gas / rho_liquid, 1e-12);
"#)
    }

    /// Generate degradation ODE equations for PEMFC
    pub fn generate_degradation_eqns() -> String {
        String::from(
            r#"// PEMFC degradation models
// Pt dissolution: d(ECSA)/dt = -k_pt * ECSA * (ε/ε_ref)^m
// Carbon corrosion: d(δ_c)/dt = k_c * exp(-Ea/R/T) * (ε/ε_ref)
// Pinhole formation: d(A_pin)/dt = k_pin * (t - t_0)

IOdictionary degradationProperties
(
    IOobject
    (
        "degradationProperties",
        runTime.constant(),
        mesh,
        IOobject::MUST_READ,
        IOobject::NO_WRITE
    )
);

// Electrochemical surface area (m²/g)
volScalarField ECSA
(
    IOobject("ECSA", runTime.timeName(), mesh),
    mesh,
    dimensionedScalar(dimArea / dimMass, 80.0)
);

// Membrane thickness (m)
volScalarField memThickness
(
    IOobject("memThickness", runTime.timeName(), mesh),
    mesh,
    dimensionedScalar(dimLength, 50e-6)
);

// Carbon support thickness (m)
volScalarField carbonThickness
(
    IOobject("carbonThickness", runTime.timeName(), mesh),
    mesh,
    dimensionedScalar(dimLength, 1e-6)
);

// Read degradation model
word degradationModel(degradationProperties.lookup("degradationModel"));

if (degradationModel == "ptDissolution" || degradationModel == "combined")
{
    // Pt dissolution: first-order in ECSA
    scalar k_pt = degradationProperties.get<scalar>("kPt");
    scalar m_pt = degradationProperties.get<scalar>("mPt");
    scalar epsilonRef = degradationProperties.get<scalar>("epsilonRef");

    volScalarField dECSA_dt = -k_pt * ECSA * pow(mag(epsilon)/epsilonRef + 1e-12, m_pt);
    ECSA += dECSA_dt * runTime.deltaT();
}

if (degradationModel == "carbonCorrosion" || degradationModel == "combined")
{
    // Carbon corrosion: Arrhenius with overpotential dependence
    scalar A_c = degradationProperties.get<scalar>("ACarbon");
    scalar Ea_c = degradationProperties.get<scalar>("EaCarbon");
    scalar epsilonRef = degradationProperties.get<scalar>("epsilonRef");
    scalar R = 8.314;

    volScalarField k_c = A_c * exp(-Ea_c / (R * T));
    volScalarField dCarbon_dt = -k_c * pow(mag(epsilon)/epsilonRef + 1e-12, 1.0);
    carbonThickness += dCarbon_dt * runTime.deltaT();
}

if (degradationModel == "pinholeFormation" || degradationModel == "combined")
{
    // Pinhole area formation
    scalar k_pin = degradationProperties.get<scalar>("kPinhole");
    scalar t_0 = degradationProperties.get<scalar>("t0Pinhole");

    scalar t = runTime.value();
    if (t > t_0)
    {
        memThickness -= k_pin * (t - t_0) * runTime.deltaT();
    }
}
"#)
    }

    /// Generate the full solver scaffold as a mapping of filename → content
    pub fn generate_scaffold(design: &SolverDesign) -> std::collections::HashMap<String, String> {
        let mut files = std::collections::HashMap::new();
        let name = &design.solver_name;

        files.insert("Make/files".to_string(), Self::generate_make_files(name));
        files.insert("Make/options".to_string(), Self::generate_make_options(design));
        files.insert(format!("{}.C", name), Self::generate_main_solver(design));
        files.insert("UEqn.H".to_string(), Self::generate_ueqn(design));
        files.insert("pEqn.H".to_string(), Self::generate_peqn(design));

        let eeqn = Self::generate_eeqn(design);
        if !eeqn.is_empty() {
            files.insert("EEqn.H".to_string(), eeqn);
        }
        let yeqn = Self::generate_yeqn(design);
        if !yeqn.is_empty() {
            files.insert("YEqn.H".to_string(), yeqn);
        }

        // PEMFC-specific files
        match design.template {
            SolverTemplate::PemfcFoam | SolverTemplate::PemfcThermalFoam | SolverTemplate::PemfcTwoPhaseFoam => {
                files.insert("epsEqn.H".to_string(), Self::generate_epseqn());
                files.insert("YEqn.H".to_string(), Self::generate_pemfc_yeqn());
                files.insert("degradation.H".to_string(), Self::generate_degradation_eqns());
            }
            _ => {}
        }
        if design.template == SolverTemplate::PemfcThermalFoam
            || design.template == SolverTemplate::PemfcTwoPhaseFoam
        {
            files.insert("EEqn.H".to_string(), Self::generate_pemfc_eeqn());
        }
        if design.template == SolverTemplate::PemfcTwoPhaseFoam {
            files.insert("saturationEqn.H".to_string(), Self::generate_saturation_eqn());
        }

        files
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_design() -> SolverDesign {
        SolverDesign {
            solver_name: "testFoam".into(),
            template: SolverTemplate::Custom,
            description: "Test solver".into(),
            modules: vec![PhysicsModule::FluidDynamics, PhysicsModule::Turbulence],
            coupling: CouplingStrategy::SegregatedSIMPLE,
            time_treatment: TimeTreatment::Steady,
            openfoam_version: "v2306".into(),
            validation_case: None,
        }
    }

    #[test]
    fn test_make_files_contains_exe() {
        let f = SolverScaffold::generate_make_files("testFoam");
        assert!(f.contains("testFoam.C"));
        assert!(f.contains("FOAM_USER_APPBIN"));
        assert!(f.contains("testFoam"));
    }

    #[test]
    fn test_make_options_simple() {
        let d = simple_design();
        let o = SolverScaffold::generate_make_options(&d);
        assert!(o.contains("finiteVolume"));
        assert!(o.contains("meshTools"));
        assert!(o.contains("turbulenceModels"));
    }

    #[test]
    fn test_make_options_with_compressible() {
        let mut d = simple_design();
        d.modules.push(PhysicsModule::Compressible);
        d.modules.push(PhysicsModule::HeatTransfer);
        let o = SolverScaffold::generate_make_options(&d);
        assert!(o.contains("fluidThermophysicalModels"));
        assert!(o.contains("fluidThermo"));
    }

    #[test]
    fn test_make_options_with_reactions() {
        let mut d = simple_design();
        d.modules.push(PhysicsModule::ChemicalReactions);
        d.modules.push(PhysicsModule::SpeciesTransport);
        let o = SolverScaffold::generate_make_options(&d);
        assert!(o.contains("combustionModels"));
        assert!(o.contains("reactionThermophysicalModels"));
    }

    #[test]
    fn test_main_solver_contains_header() {
        let d = simple_design();
        let s = SolverScaffold::generate_main_solver(&d);
        assert!(s.contains("testFoam"));
        assert!(s.contains("Test solver"));
        assert!(s.contains("simple.loop"));
        assert!(s.contains("UEqn.H"));
        assert!(s.contains("pEqn.H"));
    }

    #[test]
    fn test_main_solver_includes() {
        let d = simple_design();
        let s = SolverScaffold::generate_main_solver(&d);
        assert!(s.contains("fvCFD.H"));
        assert!(s.contains("createTime.H"));
        assert!(s.contains("createMesh.H"));
    }

    #[test]
    fn test_main_solver_compressible_includes_thermo() {
        let mut d = simple_design();
        d.modules.push(PhysicsModule::Compressible);
        let s = SolverScaffold::generate_main_solver(&d);
        assert!(s.contains("psiThermo.H"));
        assert!(s.contains("turbulentFluidThermoModel.H"));
    }

    #[test]
    fn test_main_solver_with_reactions() {
        let mut d = simple_design();
        d.modules.push(PhysicsModule::ChemicalReactions);
        d.modules.push(PhysicsModule::SpeciesTransport);
        d.modules.push(PhysicsModule::HeatTransfer);
        let s = SolverScaffold::generate_main_solver(&d);
        assert!(s.contains("combustionModel.H"));
        assert!(s.contains("YEqn.H"));
    }

    #[test]
    fn test_main_solver_convergence_check_for_simple() {
        let d = simple_design();
        let s = SolverScaffold::generate_main_solver(&d);
        assert!(s.contains("convergenceCriterion"));
    }

    #[test]
    fn test_main_solver_steady_no_ddt() {
        let d = simple_design();
        let s = SolverScaffold::generate_main_solver(&d);
        assert!(s.contains("while (simple.loop())"));
    }

    #[test]
    fn test_ueqn_incompressible() {
        let d = simple_design();
        let u = SolverScaffold::generate_ueqn(&d);
        assert!(u.contains("fvm::div(phi, U)"));
        assert!(u.contains("turbulence->divDevReff(U)"));
        assert!(!u.contains("F_EM"));
    }

    #[test]
    fn test_ueqn_with_electromagnetic() {
        let mut d = simple_design();
        d.modules.push(PhysicsModule::Electromagnetic);
        let u = SolverScaffold::generate_ueqn(&d);
        assert!(u.contains("F_EM"));
    }

    #[test]
    fn test_peqn_incompressible() {
        let d = simple_design();
        let p = SolverScaffold::generate_peqn(&d);
        assert!(p.contains("constrainHbyA"));
        assert!(p.contains("phiHbyD"));
    }

    #[test]
    fn test_peqn_compressible() {
        let mut d = simple_design();
        d.modules.push(PhysicsModule::Compressible);
        let p = SolverScaffold::generate_peqn(&d);
        assert!(p.contains("fvc::interpolate(rAU) * fvc::ddtCorr(U, phi)"));
    }

    #[test]
    fn test_eeqn_empty_without_heat_transfer() {
        let d = simple_design();
        assert!(SolverScaffold::generate_eeqn(&d).is_empty());
    }

    #[test]
    fn test_eeqn_with_heat_transfer() {
        let mut d = simple_design();
        d.modules.push(PhysicsModule::HeatTransfer);
        let e = SolverScaffold::generate_eeqn(&d);
        assert!(e.contains("fvm::ddt(rho, he)"));
        assert!(e.contains("thermo.correct()"));
    }

    #[test]
    fn test_yeqn_empty_without_species() {
        let d = simple_design();
        assert!(SolverScaffold::generate_yeqn(&d).is_empty());
    }

    #[test]
    fn test_yeqn_with_species() {
        let mut d = simple_design();
        d.modules.push(PhysicsModule::SpeciesTransport);
        let y = SolverScaffold::generate_yeqn(&d);
        assert!(y.contains("Y[i]"));
        assert!(y.contains("Sct"));
    }

    #[test]
    fn test_yeqn_with_reactions() {
        let mut d = simple_design();
        d.modules.push(PhysicsModule::ChemicalReactions);
        d.modules.push(PhysicsModule::SpeciesTransport);
        let y = SolverScaffold::generate_yeqn(&d);
        assert!(y.contains("combustion->correct()"));
        assert!(y.contains("Qdot"));
    }

    #[test]
    fn test_create_fields_incompressible() {
        let d = simple_design();
        let f = SolverScaffold::generate_create_fields(&d);
        assert!(f.contains("volVectorField U"));
        assert!(f.contains("createPhi.H"));
        assert!(f.contains("turbulentTransportModel.H"));
    }

    #[test]
    fn test_create_fields_compressible() {
        let mut d = simple_design();
        d.modules.push(PhysicsModule::Compressible);
        let f = SolverScaffold::generate_create_fields(&d);
        assert!(f.contains("psiThermo"));
        assert!(f.contains("thermo.rho()"));
    }

    #[test]
    fn test_create_fields_with_em() {
        let mut d = simple_design();
        d.modules.push(PhysicsModule::Electromagnetic);
        let f = SolverScaffold::generate_create_fields(&d);
        assert!(f.contains("volVectorField B"));
        assert!(f.contains("volScalarField phiE"));
    }

    #[test]
    fn test_create_fields_with_radiation() {
        let mut d = simple_design();
        d.modules.push(PhysicsModule::HeatTransfer);
        d.modules.push(PhysicsModule::Radiation);
        let f = SolverScaffold::generate_create_fields(&d);
        assert!(f.contains("radiationModel"));
    }

    #[test]
    fn test_generate_scaffold_returns_all_files() {
        let d = simple_design();
        let files = SolverScaffold::generate_scaffold(&d);
        assert!(files.contains_key("Make/files"));
        assert!(files.contains_key("Make/options"));
        assert!(files.contains_key("testFoam.C"));
        assert!(files.contains_key("UEqn.H"));
        assert!(files.contains_key("pEqn.H"));
        // No EEqn or YEqn without modules
        assert!(!files.contains_key("EEqn.H"));
        assert!(!files.contains_key("YEqn.H"));
    }

    #[test]
    fn test_generate_scaffold_with_thermal_species() {
        let mut d = simple_design();
        d.modules.push(PhysicsModule::HeatTransfer);
        d.modules.push(PhysicsModule::SpeciesTransport);
        let files = SolverScaffold::generate_scaffold(&d);
        assert!(files.contains_key("EEqn.H"));
        assert!(files.contains_key("YEqn.H"));
    }

    #[test]
    fn test_base_solver_name() {
        assert_eq!(SolverScaffold::base_solver_name(&SolverTemplate::MhdSimpleFoam), "simpleFoam");
        assert_eq!(SolverScaffold::base_solver_name(&SolverTemplate::HyperReactingFoam), "rhoCentralFoam");
        assert_eq!(SolverScaffold::base_solver_name(&SolverTemplate::ChtRotatingFoam), "chtMultiRegionSimpleFoam");
    }

    // ── PEMFC Scaffold Tests ───────────────────────────────────

    #[test]
    fn test_pemfc_base_solver_name() {
        assert_eq!(SolverScaffold::base_solver_name(&SolverTemplate::PemfcFoam), "simpleFoam");
        assert_eq!(SolverScaffold::base_solver_name(&SolverTemplate::PemfcThermalFoam), "buoyantSimpleFoam");
        assert_eq!(SolverScaffold::base_solver_name(&SolverTemplate::PemfcTwoPhaseFoam), "reactingTwoPhaseEulerFoam");
    }

    #[test]
    fn test_pemfc_scaffold_generates_eps_eqn() {
        let epseqn = SolverScaffold::generate_epseqn();
        assert!(epseqn.contains("sigmaMem"));
        assert!(epseqn.contains("epsilon"));
        assert!(epseqn.contains("iBV"));
        assert!(epseqn.contains("laplacian"));
    }

    #[test]
    fn test_pemfc_scaffold_generates_yeqn() {
        let yeqn = SolverScaffold::generate_pemfc_yeqn();
        assert!(yeqn.contains("Yh2"));
        assert!(yeqn.contains("Yo2"));
        assert!(yeqn.contains("Yh2o_g"));
        assert!(yeqn.contains("iBV"));
    }

    #[test]
    fn test_pemfc_scaffold_generates_eeqn() {
        let eeqn = SolverScaffold::generate_pemfc_eeqn();
        assert!(eeqn.contains("QJoule"));
        assert!(eeqn.contains("Qrxn"));
    }

    #[test]
    fn test_pemfc_scaffold_generates_saturation_eqn() {
        let seqn = SolverScaffold::generate_saturation_eqn();
        assert!(seqn.contains("s_l"));
        assert!(seqn.contains("S_w"));
    }

    #[test]
    fn test_pemfc_scaffold_generates_degradation_eqns() {
        let deg = SolverScaffold::generate_degradation_eqns();
        assert!(deg.contains("ECSA"));
        assert!(deg.contains("ptDissolution"));
        assert!(deg.contains("carbonCorrosion"));
        assert!(deg.contains("pinholeFormation"));
    }

    #[test]
    fn test_pemfc_foam_scaffold_includes_all_files() {
        let d = SolverDesign {
            solver_name: "pemfcFoam".into(),
            template: SolverTemplate::PemfcFoam,
            description: "PEMFC isothermal solver".into(),
            modules: vec![PhysicsModule::FluidDynamics, PhysicsModule::PorousMedia],
            coupling: CouplingStrategy::SegregatedSIMPLE,
            time_treatment: TimeTreatment::Steady,
            openfoam_version: "v2306".into(),
            validation_case: None,
        };
        let files = SolverScaffold::generate_scaffold(&d);
        assert!(files.contains_key("epsEqn.H"));
        assert!(files.contains_key("YEqn.H"));
        assert!(files.contains_key("degradation.H"));
        // PemfcFoam should NOT have EEqn.H or saturationEqn.H
        assert!(!files.contains_key("EEqn.H"));
        assert!(!files.contains_key("saturationEqn.H"));
    }

    #[test]
    fn test_pemfc_thermal_scaffold_includes_eeqn() {
        let d = SolverDesign {
            solver_name: "pemfcThermalFoam".into(),
            template: SolverTemplate::PemfcThermalFoam,
            description: "PEMFC non-isothermal solver".into(),
            modules: vec![PhysicsModule::FluidDynamics, PhysicsModule::HeatTransfer],
            coupling: CouplingStrategy::SegregatedSIMPLE,
            time_treatment: TimeTreatment::Steady,
            openfoam_version: "v2306".into(),
            validation_case: None,
        };
        let files = SolverScaffold::generate_scaffold(&d);
        assert!(files.contains_key("EEqn.H"));
        assert!(!files.contains_key("saturationEqn.H"));
    }

    #[test]
    fn test_pemfc_two_phase_scaffold_includes_all() {
        let d = SolverDesign {
            solver_name: "pemfcTwoPhaseFoam".into(),
            template: SolverTemplate::PemfcTwoPhaseFoam,
            description: "PEMFC two-phase solver".into(),
            modules: vec![PhysicsModule::FluidDynamics, PhysicsModule::TwoPhase, PhysicsModule::HeatTransfer],
            coupling: CouplingStrategy::SegregatedSIMPLE,
            time_treatment: TimeTreatment::UnsteadyFirstOrder,
            openfoam_version: "v2306".into(),
            validation_case: None,
        };
        let files = SolverScaffold::generate_scaffold(&d);
        assert!(files.contains_key("EEqn.H"));
        assert!(files.contains_key("saturationEqn.H"));
    }
}
