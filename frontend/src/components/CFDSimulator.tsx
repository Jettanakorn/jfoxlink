import React, { useState, useEffect, useRef } from 'react';
import { 
  Play, 
  RotateCcw, 
  Activity, 
  Wind, 
  TrendingUp, 
  AlertTriangle, 
  CheckCircle, 
  Info, 
  Sparkles,
  File,
  Layers,
  CheckCircle2,
  Cpu,
  ArrowRight,
  Sparkle
} from 'lucide-react';
import { preconfiguredAirfoils } from '../data';
import { Project } from '../types';

interface CFDSimulatorProps {
  activeProject?: Project | null;
  allProjects?: Project[];
  onSelectProject?: (project: Project | null) => void;
}

export default function CFDSimulator({ activeProject = null, allProjects = [], onSelectProject }: CFDSimulatorProps) {
  // Active Project or standard airfoil state
  const [selectedAirfoil, setSelectedAirfoil] = useState(preconfiguredAirfoils[0]);
  
  // Interactive Simulation variables
  const [angleAttack, setAngleAttack] = useState(5.0); // degrees
  const [flowSpeed, setFlowSpeed] = useState(140); // m/s
  const [chordLength, setChordLength] = useState(4.5); // meters
  const [isSimulating, setIsSimulating] = useState(true);
  
  // Real-time coefficients
  const [currentC_l, setCurrentC_l] = useState(0.65);
  const [currentC_d, setCurrentC_d] = useState(0.032);
  const [isStalled, setIsStalled] = useState(false);

  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const animationFrameRef = useRef<number | null>(null);
  const particlesRef = useRef<{ x: number; y: number; speed: number; lane: number }[]>([]);

  // 1. Sync simulation parameters when activeProject is set or changed!
  useEffect(() => {
    if (activeProject) {
      // Map project's numeric parameters into simulation settings!
      // Sweep Angle -> Maps to initial Angle of Attack gracefully (fallback to default if 0)
      const mappedAoA = activeProject.parameters.sweepAngle > 0 
        ? parseFloat((activeProject.parameters.sweepAngle * 0.25).toFixed(1)) 
        : 5.5;
      setAngleAttack(mappedAoA);

      // Mach Speed -> Maps to Wind Tunnel Velocity Speed (Mach 0.72 * 230m/s = ~165 m/s)
      const mappedSpeed = Math.round(activeProject.parameters.machNumber * 215);
      setFlowSpeed(Math.max(40, Math.min(350, mappedSpeed)));

      // Aspect Ratio -> Maps to chord span scale (Aspect Ratio 9.5 -> 5.2m)
      const mappedChord = parseFloat((activeProject.parameters.aspectRatio * 0.5 + 1.0).toFixed(1));
      setChordLength(Math.max(1.0, Math.min(9.0, mappedChord)));
    }
  }, [activeProject]);

  // 2. Update fluid dynamics coefficient maths dynamically as sliders interact
  useEffect(() => {
    // Basic aerodynamics approximation
    const isCustomCAD = activeProject && activeProject.cadFile;
    const stallThreshold = isCustomCAD ? 15.5 : (selectedAirfoil.id === 'delta-supersonic' ? 18 : 13.5);
    const isStallState = angleAttack > stallThreshold;
    setIsStalled(isStallState);

    let liftMultiplier = 1.0;
    let dragMultiplier = 1.0;

    if (isStallState) {
      // In stall, lift collapses and drag explodes
      const stallDeficit = angleAttack - stallThreshold;
      liftMultiplier = Math.max(0.25, 1.0 - (stallDeficit * 0.09));
      dragMultiplier = 1.0 + (stallDeficit * 0.18);
    } else {
      // Linear lift zone
      const optimalOffset = isCustomCAD ? 3.5 : selectedAirfoil.optAoA;
      liftMultiplier = 1.0 + (angleAttack - optimalOffset) * 0.075;
      // Parabolic drag zone
      const aoaOffset = Math.abs(angleAttack - optimalOffset);
      dragMultiplier = 1.0 + (aoaOffset * aoaOffset) * 0.016;
    }

    // Include wind speed scaling factors
    const speedRatio = flowSpeed / 140;
    const baseLiftCoef = isCustomCAD ? 0.78 : selectedAirfoil.liftCoef;
    const baseDragCoef = isCustomCAD ? 0.024 : selectedAirfoil.dragCoef;

    const computedCl = parseFloat(Math.max(0.1, baseLiftCoef * liftMultiplier * (1 + (speedRatio - 1) * 0.16)).toFixed(3));
    const computedCd = parseFloat(Math.max(0.006, baseDragCoef * dragMultiplier * (1 + (speedRatio - 1) * 0.22)).toFixed(4));
    
    setCurrentC_l(computedCl);
    setCurrentC_d(computedCd);
  }, [selectedAirfoil, angleAttack, flowSpeed, chordLength, activeProject]);

  // Handle Airfoil Selection
  const selectAirfoilById = (id: string) => {
    const air = preconfiguredAirfoils.find(a => a.id === id);
    if (air) {
      // Unselect any active project workspace to focus on generic airfoil
      if (onSelectProject) {
        onSelectProject(null);
      }
      setSelectedAirfoil(air);
      setAngleAttack(air.optAoA);
    }
  };

  // Handle Project Selection from dentro Simulator dropdown
  const selectProjectById = (id: string) => {
    if (id === 'none') {
      if (onSelectProject) onSelectProject(null);
      return;
    }
    const proj = allProjects.find(p => p.id === id);
    if (proj && onSelectProject) {
      onSelectProject(proj);
    }
  };

  // Initialize Particles system on Canvas
  useEffect(() => {
    const particles = [];
    const count = 130;
    for (let i = 0; i < count; i++) {
      particles.push({
        x: Math.random() * 550,
        y: Math.random() * 300,
        speed: 3.5 + Math.random() * 3.5,
        lane: Math.random() * 260 + 20
      });
    }
    particlesRef.current = particles;
  }, []);

  // Main Canvas animation render loop
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    let width = canvas.width;
    let height = canvas.height;

    const render = () => {
      ctx.clearRect(0, 0, width, height);

      // 1. Draw grid backdrop representing engineering alignment coordinates
      const isCustomCAD = activeProject && activeProject.cadFile;
      
      ctx.strokeStyle = '#f1f5f9';
      ctx.lineWidth = 1;
      for (let x = 0; x < width; x += 40) {
        ctx.beginPath();
        ctx.moveTo(x, 0);
        ctx.lineTo(x, height);
        ctx.stroke();
      }
      for (let y = 0; y < height; y += 40) {
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(width, y);
        ctx.stroke();
      }

      // 2. Define geometry and layout positions for Wing Segment
      const airfoilX = width / 2;
      const airfoilY = height / 2;
      const radAngle = (angleAttack * Math.PI) / 180;

      // Draw background pressure zones (cyan/emerald to denote CAD, salmon/red for stalls)
      if (isSimulating) {
        const zoneRad = isStalled ? 35 : 22 + angleAttack * 1.6;
        const fillGrad = ctx.createRadialGradient(
          airfoilX - 20, airfoilY - 40, 10,
          airfoilX, airfoilY - 30, zoneRad + 65
        );
        if (isStalled) {
          fillGrad.addColorStop(0, 'rgba(239, 68, 68, 0.14)'); // stall warning red field
          fillGrad.addColorStop(1, 'rgba(255, 255, 255, 0)');
        } else if (isCustomCAD) {
          fillGrad.addColorStop(0, 'rgba(16, 185, 129, 0.14)'); // Custom CAD dynamic green pressure profile
          fillGrad.addColorStop(1, 'rgba(255, 255, 255, 0)');
        } else {
          fillGrad.addColorStop(0, 'rgba(2, 132, 199, 0.16)'); // standard suction lift peak cyan
          fillGrad.addColorStop(1, 'rgba(255, 255, 255, 0)');
        }
        ctx.fillStyle = fillGrad;
        ctx.beginPath();
        ctx.arc(airfoilX, airfoilY - 10, zoneRad + 50, 0, Math.PI * 2);
        ctx.fill();
      }

      // Apply transformations to swing airfoil coordinates relative to AoA slider
      ctx.save();
      ctx.translate(width / 2, height / 2);
      ctx.rotate(radAngle);
      ctx.translate(-width / 2, -height / 2);

      if (isCustomCAD) {
        // --- HIGH FIDELITY 3D WIREFRAME CAD ASSEMBLY VIEWPORT ---
        // Translucent fill face
        ctx.beginPath();
        ctx.fillStyle = 'rgba(16, 185, 129, 0.08)'; // cool modern translucent emerald
        ctx.strokeStyle = '#059669'; // vivid emerald CAD wireframe guide outline
        ctx.lineWidth = 2.5;

        // Draw a detailed isometric 3D wing structure with structural sweep & rib profiles
        const midX = width / 2;
        const midY = height / 2;

        // Front vertex edge, trailing, chord, tip
        ctx.moveTo(midX - 130, midY - 12);
        ctx.bezierCurveTo(midX - 60, midY - 42, midX + 50, midY - 22, midX + 110, midY - 4);
        ctx.bezierCurveTo(midX + 60, midY + 18, midX - 30, midY + 22, midX - 130, midY - 12);
        ctx.fill();
        ctx.stroke();

        // Overlay structural isometric internal stiffener spar lines (3D representation)
        ctx.strokeStyle = 'rgba(5, 150, 105, 0.4)';
        ctx.lineWidth = 1.2;

        // Rib 1
        ctx.beginPath();
        ctx.moveTo(midX - 80, midY - 20);
        ctx.quadraticCurveTo(midX - 60, midY - 35, midX - 40, midY + 3);
        ctx.stroke();

        // Rib 2
        ctx.beginPath();
        ctx.moveTo(midX - 20, midY - 22);
        ctx.quadraticCurveTo(midX, midY - 32, midX + 20, midY + 2);
        ctx.stroke();

        // Rib 3 (near tip)
        ctx.beginPath();
        ctx.moveTo(midX + 40, midY - 17);
        ctx.quadraticCurveTo(midX + 60, midY - 25, midX + 80, midY - 2);
        ctx.stroke();

        // Spar center stringer
        ctx.strokeStyle = '#10b981';
        ctx.lineWidth = 1;
        ctx.setLineDash([2, 3]);
        ctx.beginPath();
        ctx.moveTo(midX - 140, midY - 12);
        ctx.lineTo(midX + 120, midY - 5);
        ctx.stroke();
        ctx.setLineDash([]);

        // Interactive calibration target lines on outer model
        ctx.strokeStyle = 'rgba(5, 150, 105, 0.25)';
        ctx.lineWidth = 0.8;
        ctx.beginPath();
        ctx.arc(midX - 130, midY - 12, 10, 0, Math.PI * 2);
        ctx.arc(midX + 110, midY - 4, 10, 0, Math.PI * 2);
        ctx.stroke();

      } else {
        // --- STANDARD 2D AIRFOIL MODE ---
        ctx.beginPath();
        ctx.fillStyle = '#1e293b'; // Slate wing core
        ctx.strokeStyle = '#006194'; // Primary highlight outline
        ctx.lineWidth = 2.5;

        if (selectedAirfoil.id === 'naca-4412') {
          // NACA 4412
          ctx.moveTo(width / 2 - 120, height / 2);
          ctx.quadraticCurveTo(width / 2 - 40, height / 2 - 38, width / 2 + 100, height / 2);
          ctx.quadraticCurveTo(width / 2 - 20, height / 2 + 18, width / 2 - 120, height / 2);
        } else if (selectedAirfoil.id === 'supercritical-sc02') {
          // SC-02 Supercritical Profile
          ctx.moveTo(width / 2 - 120, height / 2);
          ctx.bezierCurveTo(width / 2 - 40, height / 2 - 28, width / 2 + 20, height / 2 - 18, width / 2 + 100, height / 2);
          ctx.bezierCurveTo(width / 2 + 40, height / 2 + 16, width / 2 - 40, height / 2 + 10, width / 2 - 120, height / 2);
        } else if (selectedAirfoil.id === 'delta-supersonic') {
          // Supersonic Sharp Section
          ctx.moveTo(width / 2 - 120, height / 2);
          ctx.lineTo(width / 2 + 30, height / 2 - 10);
          ctx.lineTo(width / 2 + 100, height / 2);
          ctx.lineTo(width / 2 + 30, height / 2 + 10);
          ctx.closePath();
        } else {
          // MH 114 UAV Tall Camber
          ctx.moveTo(width / 2 - 120, height / 2);
          ctx.quadraticCurveTo(width / 2 - 30, height / 2 - 48, width / 2 + 100, height / 2);
          ctx.quadraticCurveTo(width / 2 - 10, height / 2 + 5, width / 2 - 120, height / 2);
        }
        ctx.fill();
        ctx.stroke();

        // Reference baseline dash
        ctx.strokeStyle = 'rgba(0, 97, 148, 0.25)';
        ctx.lineWidth = 1;
        ctx.setLineDash([4, 4]);
        ctx.beginPath();
        ctx.moveTo(width / 2 - 140, height / 2);
        ctx.lineTo(width / 2 + 120, height / 2);
        ctx.stroke();
        ctx.setLineDash([]);
      }

      ctx.restore();

      // 3. Render flow vector streamlines particles
      const particles = particlesRef.current;
      const speedScaling = flowSpeed / 100;

      // Streamline colors: Mint details if CAD project, Cyan for standard, Orange/Red for stall
      if (isStalled) {
        ctx.fillStyle = '#f97316'; // warnings orange
      } else if (isCustomCAD) {
        ctx.fillStyle = '#34d399'; // tech mint green
      } else {
        ctx.fillStyle = '#0284c7'; // clean aircraft blue
      }

      particles.forEach((p, idx) => {
        if (isSimulating) {
          p.x += p.speed * speedScaling;
          if (p.x > width) {
            p.x = 0;
            p.y = Math.random() * height;
          }
        }

        const dx = p.x - (width / 2);
        const dy = p.y - (height / 2);
        const dist = Math.sqrt(dx * dx + dy * dy);

        let finalY = p.y;
        
        if (dist < 185) {
          const proximityRatio = (185 - dist) / 185;
          if (isStalled) {
            // Flow Separation & turbulence bubbles
            if (p.x > width / 2 - 30) {
              finalY += Math.sin((p.x + idx * 22) * 0.12) * 16 * proximityRatio * (angleAttack / 9);
              p.x += (Math.random() - 0.2) * 1.5;
            } else {
              finalY -= (17 * proximityRatio * Math.cos(radAngle));
            }
          } else {
            // Laminar fluid deflection path
            const deflectionFactor = Math.sin(radAngle) * (isCustomCAD ? 40 : 35);
            if (p.y < height / 2) {
              finalY -= (deflectionFactor * proximityRatio * 1.22);
            } else {
              finalY += (deflectionFactor * proximityRatio * 0.82);
            }
          }
        }

        ctx.beginPath();
        const size = isStalled ? p.speed * 0.45 : p.speed * 0.65;
        ctx.arc(p.x, finalY, Math.max(1, size), 0, Math.PI * 2);
        ctx.fill();
      });

      // Bottom Chord watermark Scale
      ctx.fillStyle = '#1e293b';
      ctx.font = '10px monospace';
      ctx.fillText(`CHORD SPAN: ${chordLength}m`, width / 2 - 45, height / 2 + 65);

      // Recursive tick
      if (isSimulating) {
        animationFrameRef.current = requestAnimationFrame(render);
      }
    };

    render();

    return () => {
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current);
      }
    };
  }, [selectedAirfoil, angleAttack, flowSpeed, chordLength, isSimulating, isStalled, activeProject]);

  // List of projects containing CAD files to easily choose
  const cadProjects = allProjects.filter(p => p.cadFile);

  return (
    <div className="space-y-5 animate-fade-in">
      {/* Header and overview block */}
      <div className={`flex flex-col flex-wrap sm:flex-row sm:items-center sm:justify-between gap-3 border-b border-brand-border p-4 rounded-lg shadow-2xs ${activeProject?.cadFile ? 'bg-emerald-50/20 border-emerald-200' : 'bg-white'}`}>
        <div>
          <div className="flex items-center space-x-2">
            <Activity className={`h-5 w-5 ${activeProject?.cadFile ? 'text-emerald-600' : 'text-brand-primary'}`} />
            <h2 className="font-sans text-sm font-bold uppercase tracking-wider text-brand-text flex items-center gap-1.5">
              Computational Fluid Solver Sandbox
            </h2>
            {activeProject?.cadFile && (
              <span className="bg-emerald-100/70 text-emerald-800 border border-emerald-200 font-mono text-[9px] font-bold px-2 py-0.5 rounded uppercase tracking-wider animate-pulse">
                Custom CAD Live Mode
              </span>
            )}
          </div>
          <p className="font-mono text-[10.5px] text-brand-text-muted mt-0.5">
            {activeProject?.cadFile 
              ? `Dynamic Navier-Stokes solver configured for direct ${activeProject.cadFile.type} structural telemetry.` 
              : 'Navier-Stokes Steady Velocity Streamline Visualization (Bern Research Grid Node-8)'}
          </p>
        </div>

        {/* Master controls and reset */}
        <div className="flex items-center space-x-2">
          {isStalled && (
            <span className="animate-pulse bg-red-100 border border-red-200 text-red-700 font-bold px-2.5 py-0.5 rounded font-mono text-[10.2px] uppercase flex items-center gap-1">
              <AlertTriangle className="h-3.5 w-3.5" /> Airfoil Stall
            </span>
          )}
          <button
            onClick={() => setIsSimulating(!isSimulating)}
            className={`rounded-md px-3.5 py-1.5 font-sans text-xs font-bold text-white shadow-xs transition-colors flex items-center gap-1.5 cursor-pointer ${
              isSimulating 
                ? 'bg-amber-600 hover:bg-amber-700' 
                : (activeProject?.cadFile ? 'bg-emerald-600 hover:bg-emerald-700' : 'bg-brand-primary hover:bg-brand-accent')
            }`}
          >
            <Play className={`h-3.5 w-3.5 ${isSimulating ? 'animate-pulse' : ''}`} />
            <span>{isSimulating ? 'Pause Solves' : 'Resume Solves'}</span>
          </button>
        </div>
      </div>

      {/* Main Sandbox Grid */}
      <div className="grid grid-cols-1 gap-5 lg:grid-cols-12">
        {/* Left Column: Airfoils and parameters modifiers */}
        <div className="space-y-5 lg:col-span-4">
          
          {/* Custom CAD project loaded alert & details */}
          {activeProject && (
            <div className="rounded-lg border border-emerald-200 bg-emerald-50/20 p-4 space-y-3.5 shadow-2xs">
              <div className="flex items-center justify-between border-b border-emerald-100/60 pb-2">
                <span className="font-sans font-bold text-[10.5px] uppercase tracking-wide text-emerald-800 flex items-center gap-1.5">
                  <Cpu className="h-4 w-4 text-emerald-600" /> Loaded CAD Project
                </span>
                <button 
                  onClick={() => onSelectProject?.(null)}
                  className="font-sans text-[10px] text-emerald-700 hover:text-emerald-900 font-semibold hover:underline"
                >
                  Clear Workspace
                </button>
              </div>

              <div className="space-y-1.5 text-left">
                <h4 className="font-sans text-xs font-bold text-brand-text truncate">
                  {activeProject.name}
                </h4>
                <p className="font-sans text-[11px] text-brand-text-muted leading-relaxed line-clamp-2">
                  {activeProject.description}
                </p>
              </div>

              {activeProject.cadFile && (
                <div className="rounded bg-white p-2.5 border border-emerald-100 flex items-center justify-between font-sans text-xs">
                  <div className="flex items-center space-x-2 min-w-0">
                    <File className="h-4 w-4 text-emerald-600 shrink-0" />
                    <div className="min-w-0 overflow-hidden">
                      <p className="font-bold text-brand-text truncate text-[11px]">
                        {activeProject.cadFile.name}
                      </p>
                      <p className="font-mono text-[9px] text-brand-text-muted">
                        {activeProject.cadFile.size} • {activeProject.cadFile.type.toUpperCase()} File
                      </p>
                    </div>
                  </div>
                  <span className="font-mono text-[9px] text-emerald-700 bg-emerald-100/50 px-1.5 py-0.5 rounded font-bold shrink-0">
                    ACTIVE
                  </span>
                </div>
              )}

              <div className="bg-white/80 rounded p-2 text-center text-[10px] font-mono text-emerald-700 border border-emerald-50 text-[10.5px]">
                🚀 Parameters preloaded automatically!
              </div>
            </div>
          )}

          {/* Quick-select loaded workspace project with CAD files */}
          {allProjects.length > 0 && (
            <div className="rounded-lg border border-brand-border bg-white p-4 space-y-3 shadow-2xs">
              <span className="font-sans font-bold text-[10px] uppercase tracking-wide text-brand-text-muted flex items-center gap-1">
                <Layers className="h-3.5 w-3.5 text-brand-text-muted" /> Switch Project Workspace
              </span>
              <div>
                <select
                  value={activeProject?.id || 'none'}
                  onChange={(e) => selectProjectById(e.target.value)}
                  className="w-full rounded border border-brand-border bg-brand-bg px-2.5 py-1.5 font-sans text-xs text-brand-text outline-hidden cursor-pointer"
                >
                  <option value="none">--- Use Free sandbox Airfoils ---</option>
                  {allProjects.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.name} {p.cadFile ? `(CAD: ${p.cadFile.name})` : '(No CAD)'}
                    </option>
                  ))}
                </select>
              </div>
            </div>
          )}

          {/* Standard airfoils selection widget */}
          <div className="rounded-lg border border-brand-border bg-white p-4 space-y-3 shadow-2xs">
            <span className="font-sans font-bold text-[10px] uppercase tracking-wide text-brand-text-muted flex items-center gap-1">
              <Sparkles className="h-3.5 w-3.5 text-brand-primary" /> Standard Flight Profiles
            </span>
            <div className="space-y-2 max-h-56 overflow-y-auto pr-1">
              {preconfiguredAirfoils.map((air) => {
                const isSelected = !activeProject && selectedAirfoil.id === air.id;
                return (
                  <div
                    key={air.id}
                    onClick={() => selectAirfoilById(air.id)}
                    className={`cursor-pointer rounded border p-2.5 transition-all text-left ${
                      isSelected 
                        ? 'border-brand-primary bg-brand-container-low/40 ring-1 ring-brand-primary' 
                        : 'border-brand-bg hover:bg-brand-bg hover:border-brand-border'
                    }`}
                  >
                    <span className="block font-sans text-xs font-bold text-brand-text leading-tight">{air.name}</span>
                    <p className="mt-0.5 line-clamp-2 font-sans text-[10.5px] text-brand-text-muted leading-tight">{air.description}</p>
                  </div>
                );
              })}
            </div>
          </div>

          {/* Flow parameters sliders section */}
          <div className="rounded-lg border border-brand-border bg-white p-4 space-y-4 shadow-2xs">
            <span className="font-sans font-bold text-[10px] uppercase tracking-wide text-brand-text-muted flex items-center gap-1">
              <Wind className="h-3.5 w-3.5 text-brand-primary" /> Wind Tunnel Fluid Parameters
            </span>

            {/* Angle of Attack (AoA) */}
            <div className="space-y-1">
              <div className="flex items-center justify-between font-sans text-xs font-semibold text-brand-text">
                <span>Angle of Attack</span>
                <span className={`font-mono text-xs font-bold ${isStalled ? 'text-red-650 text-red-650 text-red-650 text-red-650 text-red-650 animate-pulse text-red-650 font-black' : 'text-brand-primary'}`}>
                  {angleAttack}°
                </span>
              </div>
              <input
                type="range"
                min="-10.0"
                max="25.0"
                step="0.5"
                value={angleAttack}
                onChange={(e) => setAngleAttack(parseFloat(e.target.value))}
                className="w-full accent-brand-primary cursor-ew-resize"
              />
              <div className="flex justify-between text-[10px] text-brand-text-muted">
                <span>Downward (-10°)</span>
                <span>Stall Threshold ({activeProject?.cadFile ? '15.5°+' : '13.5°+'})</span>
                <span>Upward (25°)</span>
              </div>
            </div>

            {/* Flow Speed */}
            <div className="space-y-1 border-t pt-3">
              <div className="flex items-center justify-between font-sans text-xs font-semibold text-brand-text">
                <span>Chamber Flow Velocity</span>
                <span className="font-mono text-xs font-bold text-brand-primary">{flowSpeed} m/s</span>
              </div>
              <input
                type="range"
                min="40"
                max="350"
                step="5"
                value={flowSpeed}
                onChange={(e) => setFlowSpeed(parseInt(e.target.value))}
                className="w-full accent-brand-primary cursor-ew-resize"
              />
              <div className="flex justify-between text-[10px] text-brand-text-muted">
                <span>Subsonic (40)</span>
                <span>Transonic (250)</span>
                <span>Supersonic Speed (350)</span>
              </div>
            </div>

            {/* Chord Span length */}
            <div className="space-y-1 border-t pt-3">
              <div className="flex items-center justify-between font-sans text-xs font-semibold text-brand-text">
                <span>Physical Wing Chord Span</span>
                <span className="font-mono text-xs font-bold text-brand-primary">{chordLength} m</span>
              </div>
              <input
                type="range"
                min="1.0"
                max="9.0"
                step="0.2"
                value={chordLength}
                onChange={(e) => setChordLength(parseFloat(e.target.value))}
                className="w-full accent-brand-primary cursor-ew-resize"
              />
            </div>
          </div>
        </div>

        {/* Right Column: HTML Canvas dynamic visualization and metrics */}
        <div className="space-y-5 lg:col-span-8">
          {/* Canvas visualization stage container */}
          <div className={`rounded-lg border bg-white overflow-hidden shadow-2xs relative ${activeProject?.cadFile ? 'border-emerald-300' : 'border-brand-border'}`}>
            {/* Top coordinate trace info banner */}
            <div className="flex items-center justify-between bg-brand-bg px-4 py-2 border-b border-brand-container-low font-mono text-[9px] text-brand-text-muted">
              <span>ACTIVE MODEL: {activeProject?.cadFile ? `${activeProject.cadFile.name} (${activeProject.cadFile.type})` : selectedAirfoil.name}</span>
              <span className="flex items-center gap-1">SOLVER STATUS: <strong className={isStalled ? 'text-red-650 text-red-600' : 'text-emerald-600'}>{isStalled ? 'SEPARATED / STALL' : 'CONVERGED LAMINAR'}</strong></span>
            </div>

            {/* Canvas stage itself */}
            <div className="w-full flex justify-center bg-slate-950 py-3 relative">
              
              {/* Futuristic CAD overlay overlay on top of screen */}
              <div className="absolute top-4 left-6 pointer-events-none font-mono text-[9.5px] uppercase tracking-wider space-y-1 text-emerald-400 select-none">
                <div className="flex items-center gap-1">
                  <span className="inline-block h-1.5 w-1.5 rounded-full bg-emerald-400 animate-ping"></span>
                  <span>[ SYSTEM STATE: SOLVING ]</span>
                </div>
                {activeProject?.cadFile && (
                  <>
                    <div className="text-white">PROJECT: {activeProject.name}</div>
                    <div className="text-gray-400">GEOMETRY: {activeProject.cadFile.name}</div>
                    <div className="text-gray-400">FORMAT: {activeProject.cadFile.type} FILE</div>
                  </>
                )}
                <div className="text-slate-500">RESIDUAL TOLERANCE: 1.0e-6</div>
              </div>

              {/* Viewport helper overlay top-right */}
              <div className="absolute top-4 right-6 pointer-events-none font-mono text-[9px] text-slate-400 uppercase text-right select-none space-y-1">
                <div>GRID NODES: 48,520 HEX-CELLS</div>
                <div>TURBULENCE: SST K-OMEGA</div>
                <div className="text-emerald-400 font-bold">READY FOR FLUID RUN</div>
              </div>

              <canvas
                ref={canvasRef}
                width={550}
                height={300}
                className="max-w-full aspect-video border border-slate-800 rounded-md bg-slate-900 shadow-xl"
              />
            </div>

            {/* Bottom canvas helper stats */}
            <div className="bg-brand-bg p-3.5 border-t border-brand-container-low font-sans text-xs text-brand-text-muted flex items-start gap-1">
              <Info className={`h-4.5 w-4.5 shrink-0 mt-0.5 ${activeProject?.cadFile ? 'text-emerald-600' : 'text-brand-primary'}`} />
              <span>
                {isStalled 
                  ? 'Critical flow separation. Boundary streamlines detached generating severe deceleration eddies and massive pressure stall index. Adjust Angle of Attack to lower values to secure laminar attachment.'
                  : activeProject?.cadFile 
                  ? `Fluid streamlines adhering perfectly to ${activeProject.cadFile.name} custom curvature layout. Zero trailing stress pockets detected inside active wind chamber zone.`
                  : 'Sustained Bernoulli pressure differential. Suction and compression levels conform to optimized flow envelopes.'}
              </span>
            </div>
          </div>

          {/* Engineering Monitors */}
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {/* Cl Monitor */}
            <div className={`rounded-lg border bg-white p-3.5 shadow-2xs ${activeProject?.cadFile ? 'border-emerald-250 border-emerald-100' : 'border-brand-border'}`}>
              <span className="font-sans text-[10.5px] font-bold text-brand-text-muted uppercase tracking-wide block">Lift Coefficient (Cl)</span>
              <div className="mt-1 font-mono text-2xl font-bold text-brand-text leading-none flex items-baseline gap-1.5">
                {currentC_l}
                <span className="font-sans text-[10px] font-normal text-brand-text-muted">unitless CL</span>
              </div>
              <p className="mt-1.5 font-sans text-[10px] text-brand-text-muted">Vertical pressure aerodynamic suction peak.</p>
            </div>

            {/* Cd Monitor */}
            <div className={`rounded-lg border bg-white p-3.5 shadow-2xs ${activeProject?.cadFile ? 'border-emerald-250 border-emerald-100' : 'border-brand-border'}`}>
              <span className="font-sans text-[10.5px] font-bold text-brand-text-muted uppercase tracking-wide block">Drag Coefficient (Cd)</span>
              <div className="mt-1 font-mono text-2xl font-bold text-brand-text leading-none flex items-baseline gap-1.5">
                {currentC_d}
                <span className="font-sans text-[10px] font-normal text-brand-text-muted">unitless CD</span>
              </div>
              <p className="mt-1.5 font-sans text-[10px] text-brand-text-muted">Frontal compression friction wave ratio.</p>
            </div>

            {/* Efficiency Ratio Cl/Cd */}
            <div className={`rounded-lg border bg-white p-3.5 shadow-2xs sm:col-span-2 lg:col-span-1 ${activeProject?.cadFile ? 'border-emerald-250 border-emerald-300' : 'border-brand-border'}`}>
              <span className="font-sans text-[10.5px] font-bold text-brand-text-muted uppercase tracking-wide block">Ultimate L/D Ratio</span>
              <div className={`mt-1 font-mono text-2xl font-bold leading-none flex items-baseline gap-1.5 ${activeProject?.cadFile ? 'text-emerald-600' : 'text-brand-primary'}`}>
                {(currentC_l / currentC_d).toFixed(1)}
                <span className="font-sans text-[10px] font-normal text-brand-text-muted">Cl / Cd</span>
              </div>
              <p className="mt-1.5 font-sans text-[10px] text-brand-text-muted">Total aerodynamic glide efficiency index.</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
