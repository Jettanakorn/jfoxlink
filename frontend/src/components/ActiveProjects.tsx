import React, { useState } from 'react';
import { 
  Folder, 
  Plus, 
  X, 
  ArrowRight, 
  Cpu, 
  Layers, 
  Users, 
  Clock, 
  CheckCircle2, 
  Play, 
  Activity, 
  Sparkles, 
  TrendingUp, 
  Download,
  Terminal,
  ChevronRight,
  Upload,
  File,
  Trash2
} from 'lucide-react';
import { Project, AISuggestion, calcLiftDragRatio } from '../types';

interface ActiveProjectsProps {
  projects: Project[];
  onAddProject: (p: Project) => void;
  onUpdateProject: (p: Project) => void;
  onApplyAISuggestion: (projId: string, suggestionId: string) => void;
  onSelectProjectForDetails: (p: Project) => void;
}

export default function ActiveProjects(props: ActiveProjectsProps) {
  const [showInitModal, setShowInitModal] = useState(false);
  const [initName, setInitName] = useState('');
  const [initVersion, setInitVersion] = useState('v1.0.0');
  const [initDesc, setInitDesc] = useState('');
  const [initStatus, setInitStatus] = useState<'active' | 'stable' | 'archived'>('active');
  const [initTeammates, setInitTeammates] = useState('AT, JC');

  // CAD Upload states
  const [uploadedCadFile, setUploadedCadFile] = useState<{ name: string; size: string; type: string; uploadedAt: string } | null>(null);
  const [isDragging, setIsDragging] = useState(false);
  const [fileError, setFileError] = useState<string | null>(null);

  // Drag and Drop handlers
  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(true);
  };

  const handleDragLeave = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
  };

  const processFile = (file: File) => {
    setFileError(null);
    const extension = file.name.split('.').pop()?.toLowerCase();
    const allowedExtensions = ['step', 'stp', 'iges', 'igs', 'stl', 'obj', 'f3d', 'cad', 'dwg', 'dxf'];
    
    if (extension && !allowedExtensions.includes(extension)) {
      setFileError(`Format .${extension} is unverified. Please upload a valid 3D CAD design (.step, .stp, .iges, .stl, .obj, .f3d).`);
      return;
    }

    const sizeInMB = (file.size / (1024 * 1024)).toFixed(2);
    const displaySize = parseFloat(sizeInMB) < 0.1 ? `${(file.size / 1024).toFixed(1)} KB` : `${sizeInMB} MB`;
    
    setUploadedCadFile({
      name: file.name,
      size: displaySize,
      type: extension ? extension.toUpperCase() : 'CAD',
      uploadedAt: 'just now'
    });
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
    if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
      processFile(e.dataTransfer.files[0]);
    }
  };

  const handleFileChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && e.target.files.length > 0) {
      processFile(e.target.files[0]);
    }
  };

  const handleRemoveCadFile = () => {
    setUploadedCadFile(null);
    setFileError(null);
  };
  
  // Slide out drawer state of selected project
  const [drawerProject, setDrawerProject] = useState<Project | null>(null);
  const [isSolving, setIsSolving] = useState(false);
  const [solveProgress, setSolveProgress] = useState(0);
  const [customAIQuery, setCustomAIQuery] = useState('');
  const [aiResponseList, setAiResponseList] = useState<{ query: string; answer: string }[]>([]);

  // Simulation parameters for initialized/edited project
  const [initAspect, setInitAspect] = useState(8.5);
  const [initReynolds, setInitReynolds] = useState(12.0);
  const [initSweep, setInitSweep] = useState(25.0);
  const [initMach, setInitMach] = useState(0.72);

  const handleInitSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!initName.trim()) return;

    // Standardized lift drag ratio estimate from parameters
    const calcLiftDrag = calcLiftDragRatio(initAspect, initMach, initSweep);

    const newProject: Project = {
      id: `proj-${Date.now()}`,
      name: initName,
      version: initVersion || 'v1.0.0',
      description: initDesc || 'Custom aerodynamic boundary-layer computational fluid dynamics study.',
      status: initStatus,
      teammates: initTeammates.split(',').map(s => s.trim().toUpperCase()).filter(Boolean),
      updatedAt: 'Updated just now',
      parameters: {
        aspectRatio: initAspect,
        reynoldsNumber: initReynolds,
        sweepAngle: initSweep,
        machNumber: initMach,
        liftDragRatio: calcLiftDrag,
      },
      suggestions: [
        {
          id: `sug-${Date.now()}-1`,
          category: 'mesh',
          title: 'Fine-tune shock boundary mesh parameters',
          description: 'Subdivide boundary layers with y-plus < 1 to capture transonic separation accurately.',
          impact: '+1.4% L/D ratio stability',
          applied: false,
          parameterAffected: 'liftDragRatio',
          deltaValue: 0.25,
        }
      ],
      convergenceData: [
        { iteration: 100, lift: 0.25, drag: 0.06 },
        { iteration: 200, lift: 0.38, drag: 0.045 },
        { iteration: 300, lift: 0.45, drag: 0.038 },
        { iteration: 400, lift: 0.49, drag: 0.035 },
      ],
      cadFile: uploadedCadFile || undefined
    };

    props.onAddProject(newProject);
    if (uploadedCadFile) {
      props.onSelectProjectForDetails(newProject);
    }
    setShowInitModal(false);
    
    // Reset Form
    setInitName('');
    setInitVersion('v1.0.0');
    setInitDesc('');
    setInitTeammates('AT, JC');
    setInitAspect(8.5);
    setInitReynolds(12.0);
    setInitSweep(25.0);
    setInitMach(0.72);
    setUploadedCadFile(null);
    setFileError(null);
  };

  const handleDrawerSaveParams = (p: Project) => {
    props.onUpdateProject(p);
    // Pulse mesh solution to update graph values
    setIsSolving(true);
    setSolveProgress(0);
    const interval = setInterval(() => {
      setSolveProgress(prev => {
        if (prev >= 100) {
          clearInterval(interval);
          setTimeout(() => setIsSolving(false), 600);
          return 100;
        }
        return prev + 10;
      });
    }, 80);
  };

  const handleApplySuggestion = (proj: Project, sug: AISuggestion) => {
    props.onApplyAISuggestion(proj.id, sug.id);
    
    // Update active drawer project UI
    const updatedSug = proj.suggestions.map(s => s.id === sug.id ? { ...s, applied: true } : s);
    const impactDelta = sug.deltaValue;
    const newLiftDrag = parseFloat((proj.parameters.liftDragRatio + impactDelta).toFixed(2));
    
    const updatedProj: Project = {
      ...proj,
      parameters: {
        ...proj.parameters,
        liftDragRatio: newLiftDrag
      },
      suggestions: updatedSug,
      // Append a convergent iteration
      convergenceData: [
        ...proj.convergenceData,
        { 
          iteration: proj.convergenceData[proj.convergenceData.length - 1].iteration + 100, 
          lift: parseFloat((proj.convergenceData[proj.convergenceData.length - 1].lift * 1.015).toFixed(3)),
          drag: parseFloat((proj.convergenceData[proj.convergenceData.length - 1].drag * 0.98).toFixed(4))
        }
      ]
    };
    
    setDrawerProject(updatedProj);
    handleDrawerSaveParams(updatedProj);
  };

  const runAerodynamicAIAdvisor = () => {
    if (!customAIQuery.trim() || !drawerProject) return;
    
    // Formulate a beautiful, highly intelligent response about CFD and aerodynamics
    let answer = '';
    const q = customAIQuery.toLowerCase();
    
    if (q.includes('wing') || q.includes('lift') || q.includes('drag')) {
      answer = `To maximize L/D on ${drawerProject.name}, consider increasing the aspect ratio from ${drawerProject.parameters.aspectRatio} to ${(drawerProject.parameters.aspectRatio * 1.1).toFixed(1)} while employing supercritical sectioning SC(2) along the mid-span. This reduces early wave drag and maintains high Cl values and delays shock stall past Mach ${drawerProject.parameters.machNumber}.`;
    } else if (q.includes('mesh') || q.includes('refine') || q.includes('grid')) {
      answer = `Grid convergence verification for ${drawerProject.name} suggests refining boundary inflation layers around the leading-edge suction peak. Reducing the initial cell height (Y-Plus < 0.8) using k-omega SST solvers will minimize viscosity approximation errors at high Reynolds numbers (${drawerProject.parameters.reynoldsNumber}M).`;
    } else if (q.includes('sweep') || q.includes('angle')) {
      answer = `A sweep angle of ${drawerProject.parameters.sweepAngle}° is optimal for delaying wave compression at Mach ${drawerProject.parameters.machNumber}. If your flight envelope exceeds Mach ${(drawerProject.parameters.machNumber * 1.05).toFixed(2)}, sweep must be expanded by 2.5° to constrain shockwave attachment boundaries.`;
    } else {
      answer = `Aerodynamic Analysis Report [${drawerProject.name} Simulation Profile]: Recommended modification is to optimize the trailing edge flap reflex curvature at 75% chord span. This maintains laminar boundary transitions and yields an estimated +1.8% Lift-to-Drag ratio improvement.`;
    }
    
    setAiResponseList(prev => [...prev, { query: customAIQuery, answer }]);
    setCustomAIQuery('');
  };

  return (
    <div id="active-projects-container" className="space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="font-sans text-sm font-bold uppercase tracking-wider text-brand-text">
          Active Projects
        </h3>
        <button
          onClick={() => {
            // Setup pre-filled mock details
            setInitName(`Project ${['Epsilon', 'Omega', 'Valkyrie', 'Sirocco'][Math.floor(Math.random() * 4)]}-${Math.floor(Math.random() * 9 + 1)}`);
            setInitDesc('Supersonic boundary compression airflow and shockwaves interaction modeling.');
            setShowInitModal(true);
          }}
          className="text-xs font-bold text-brand-primary hover:text-brand-accent hover:underline cursor-pointer"
        >
          View All Projects
        </button>
      </div>

      {/* Grid of Projects */}
      <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
        {props.projects.map((proj) => (
          <div
            key={proj.id}
            onClick={() => {
              setDrawerProject(proj);
              setAiResponseList([]);
            }}
            className="group relative cursor-pointer rounded-lg border border-brand-border bg-white p-4 transition-all hover:bg-brand-container-low/30 hover:shadow-xs"
          >
            {/* Upper Info Row */}
            <div className="flex items-start justify-between">
              <div className="flex items-center space-x-2.5">
                <div className="flex h-8 w-8 items-center justify-center rounded-md bg-brand-container text-brand-primary group-hover:bg-brand-primary group-hover:text-white transition-colors duration-200">
                  <Folder className="h-4.5 w-4.5" />
                </div>
                <div>
                  <h4 className="font-sans text-xs font-bold text-brand-text group-hover:text-brand-primary transition-colors">
                    {proj.name}
                  </h4>
                  <p className="mt-0.5 line-clamp-2 font-sans text-[11px] text-brand-text-muted leading-relaxed">
                    {proj.description}
                  </p>
                  {proj.cadFile && (
                    <div className="mt-2 flex items-center space-x-1.5 text-[10px] text-emerald-700 bg-emerald-50/70 border border-emerald-100 rounded px-1.5 py-0.5 w-fit font-mono">
                      <span className="relative flex h-1.5 w-1.5">
                        <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75"></span>
                        <span className="relative inline-flex rounded-full h-1.5 w-1.5 bg-emerald-500"></span>
                      </span>
                      <span className="truncate max-w-[200px]">CAD: {proj.cadFile.name} ({proj.cadFile.size})</span>
                    </div>
                  )}
                </div>
              </div>

              {/* Version/Status Badges */}
              <span className={`rounded px-1.5 py-0.5 font-sans text-[10px] font-bold uppercase ${
                proj.status === 'active' 
                  ? 'bg-brand-container text-brand-primary' 
                  : proj.status === 'stable'
                  ? 'bg-emerald-50 text-emerald-700 border border-emerald-100'
                  : 'bg-brand-container-high text-brand-text-muted'
              }`}>
                {proj.version}
              </span>
            </div>

            {/* Bottom Metadata Info */}
            <div className="mt-4 flex items-center justify-between border-t border-brand-bg pt-3 text-[11px] text-brand-text-muted">
              {/* Teammates Initials Stack */}
              <div className="flex items-center space-x-1.5">
                <div className="flex -space-x-1.5 overflow-hidden">
                  {proj.teammates.slice(0, 2).map((tm, idx) => (
                    <div 
                      key={idx} 
                      className={`flex h-5 w-5 items-center justify-center rounded-full text-[9px] font-bold text-white border border-white uppercase ${
                        idx === 0 ? 'bg-brand-primary' : 'bg-brand-accent'
                      }`}
                    >
                      {tm}
                    </div>
                  ))}
                  {proj.teammates.length > 2 && (
                    <div className="flex h-5 w-5 items-center justify-center rounded-full bg-brand-container-high text-[9px] font-bold text-brand-primary border border-white">
                      +{proj.teammates.length - 2}
                    </div>
                  )}
                </div>
                <span className="font-sans text-[10px] text-brand-text-muted">
                  {proj.teammates.length} Lead Solvers
                </span>
              </div>

              {/* Time stamp */}
              <span className="font-sans text-[10px] text-brand-text-muted">
                {proj.updatedAt}
              </span>
            </div>
          </div>
        ))}

        {/* Initialize New Project Container */}
        <div
          onClick={() => {
            setInitName(`Project Omega-9`);
            setInitDesc('Secondary shockwave detachment boundaries validation at High Reynolds multi-node solving.');
            setShowInitModal(true);
          }}
          className="flex h-36 cursor-pointer flex-col items-center justify-center rounded-lg border border-dashed border-brand-border bg-white transition-all hover:border-brand-primary hover:bg-brand-container-low/20"
        >
          <div className="flex h-9 w-9 items-center justify-center rounded-full bg-brand-bg text-brand-text-muted transition-transform group-hover:scale-110">
            <Plus className="h-5 w-5 text-brand-text-muted" />
          </div>
          <span className="mt-2 font-sans text-xs font-bold text-brand-text">
            Initialize New Project
          </span>
          <p className="mt-0.5 font-sans text-[10px] text-brand-text-muted">
            Configure boundaries, meshing, and simulation factors
          </p>
        </div>
      </div>

      {/* Project Details Modal Drawer */}
      {drawerProject && (
        <div className="fixed inset-y-0 right-0 z-50 flex w-full max-w-2xl bg-white shadow-2xl border-l border-brand-border animate-slide-in">
          <div className="flex flex-col h-full w-full justify-between">
            {/* Drawer Header */}
            <div className="flex items-center justify-between border-b border-brand-bg px-6 py-4 bg-brand-bg">
              <div className="flex items-center space-x-3">
                <div className="rounded-md bg-brand-primary p-1.5 text-white">
                  <Folder className="h-5 w-5" />
                </div>
                <div>
                  <h3 className="font-sans text-sm font-bold text-brand-text">{drawerProject.name} Parameters</h3>
                  <span className="font-mono text-[10px] text-brand-primary">{drawerProject.version} — COMPUTATIONAL ENVELOPE</span>
                </div>
              </div>
              <button 
                onClick={() => setDrawerProject(null)}
                className="rounded-md p-1.5 hover:bg-white border border-transparent hover:border-brand-border transition-all"
              >
                <X className="h-4.5 w-4.5 text-brand-text-muted" />
              </button>
            </div>

            {/* Drawer Scrollable Content */}
            <div className="flex-1 overflow-y-auto px-6 py-4 space-y-6">
              {/* Project Description */}
              <div>
                <span className="font-mono text-[10px] font-bold text-brand-text-muted uppercase tracking-wider">Project Scope</span>
                <p className="mt-1 font-sans text-xs text-brand-text leading-relaxed bg-brand-bg rounded-md p-3 border border-brand-container-low">
                  {drawerProject.description}
                </p>
              </div>

              {/* Uploaded CAD File details */}
              {drawerProject.cadFile && (
                <div className="rounded-lg border border-emerald-100 bg-emerald-50/15 p-3.5 space-y-2">
                  <span className="font-mono text-[10px] font-bold text-emerald-800 uppercase tracking-wider flex items-center gap-1.5">
                    <CheckCircle2 className="h-3.5 w-3.5 text-emerald-600" /> Attached 3D CAD Geometry
                  </span>
                  <div className="flex items-center justify-between">
                    <div className="flex items-center space-x-2.5">
                      <div className="flex h-8 w-8 items-center justify-center rounded-md bg-emerald-100/50 text-emerald-700">
                        <File className="h-4 w-4" />
                      </div>
                      <div>
                        <p className="font-sans text-xs font-bold text-brand-text truncate max-w-[280px]">
                          {drawerProject.cadFile.name}
                        </p>
                        <p className="font-mono text-[9.5px] text-brand-text-muted">
                          {drawerProject.cadFile.size} • format: {drawerProject.cadFile.type.toUpperCase()}
                        </p>
                      </div>
                    </div>
                    <span className="font-mono text-[9px] text-brand-text-muted bg-emerald-100/40 px-1.5 py-0.5 rounded">
                      VERIFIED CFD READY
                    </span>
                  </div>
                </div>
              )}

              {/* Aerodynamics Slider Manipulators */}
              <div className="space-y-3.5 bg-brand-bg/40 rounded-lg border border-brand-container-low p-4">
                <div className="flex items-center justify-between">
                  <span className="font-mono text-[10px] font-bold text-brand-text uppercase tracking-wider flex items-center gap-1">
                    <Activity className="h-3.5 w-3.5 text-brand-primary" /> Boundary Factors Setup
                  </span>
                  <span className="font-sans text-[10px] text-brand-text-muted italic">Move slide to adjust, updates L/D mathematically</span>
                </div>

                <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                  {/* Aspect Ratio */}
                  <div>
                    <label className="flex justify-between font-sans text-xs font-semibold text-brand-text">
                      <span>Aspect Ratio</span>
                      <span className="font-mono text-brand-primary font-bold">{drawerProject.parameters.aspectRatio}</span>
                    </label>
                    <input
                      type="range"
                      min="1.0"
                      max="15.0"
                      step="0.1"
                      value={drawerProject.parameters.aspectRatio}
                      onChange={(e) => {
                        const val = parseFloat(e.target.value);
                        const updated: Project = {
                          ...drawerProject,
                          parameters: {
                            ...drawerProject.parameters,
                            aspectRatio: val,
                            // Recalculate Lift/drag score mathematically
                            liftDragRatio: calcLiftDragRatio(val, drawerProject.parameters.machNumber, drawerProject.parameters.sweepAngle)
                          }
                        };
                        setDrawerProject(updated);
                        props.onUpdateProject(updated);
                      }}
                      className="w-full accent-brand-primary"
                    />
                  </div>

                  {/* Reynolds Number */}
                  <div>
                    <label className="flex justify-between font-sans text-xs font-semibold text-brand-text">
                      <span>Reynolds Number</span>
                      <span className="font-mono text-brand-primary font-bold">{drawerProject.parameters.reynoldsNumber}M</span>
                    </label>
                    <input
                      type="range"
                      min="1.0"
                      max="30.0"
                      step="0.5"
                      value={drawerProject.parameters.reynoldsNumber}
                      onChange={(e) => {
                        const val = parseFloat(e.target.value);
                        const updated = {
                          ...drawerProject,
                          parameters: { ...drawerProject.parameters, reynoldsNumber: val }
                        };
                        setDrawerProject(updated);
                        props.onUpdateProject(updated);
                      }}
                      className="w-full accent-brand-primary"
                    />
                  </div>

                  {/* Sweep Angle */}
                  <div>
                    <label className="flex justify-between font-sans text-xs font-semibold text-brand-text">
                      <span>Sweep Angle (deg)</span>
                      <span className="font-mono text-brand-primary font-bold">{drawerProject.parameters.sweepAngle}°</span>
                    </label>
                    <input
                      type="range"
                      min="0"
                      max="60"
                      step="0.5"
                      value={drawerProject.parameters.sweepAngle}
                      onChange={(e) => {
                        const val = parseFloat(e.target.value);
                        const updated: Project = {
                          ...drawerProject,
                          parameters: {
                            ...drawerProject.parameters,
                            sweepAngle: val,
                            liftDragRatio: calcLiftDragRatio(drawerProject.parameters.aspectRatio, drawerProject.parameters.machNumber, val)
                          }
                        };
                        setDrawerProject(updated);
                        props.onUpdateProject(updated);
                      }}
                      className="w-full accent-brand-primary"
                    />
                  </div>

                  {/* Mach Cruise Speed */}
                  <div>
                    <label className="flex justify-between font-sans text-xs font-semibold text-brand-text">
                      <span>Cruise Mach Speed</span>
                      <span className="font-mono text-brand-primary font-bold">M {drawerProject.parameters.machNumber}</span>
                    </label>
                    <input
                      type="range"
                      min="0.1"
                      max="2.5"
                      step="0.01"
                      value={drawerProject.parameters.machNumber}
                      onChange={(e) => {
                        const val = parseFloat(e.target.value);
                        const updated: Project = {
                          ...drawerProject,
                          parameters: {
                            ...drawerProject.parameters,
                            machNumber: val,
                            liftDragRatio: calcLiftDragRatio(drawerProject.parameters.aspectRatio, val, drawerProject.parameters.sweepAngle)
                          }
                        };
                        setDrawerProject(updated);
                        props.onUpdateProject(updated);
                      }}
                      className="w-full accent-brand-primary"
                    />
                  </div>
                </div>

                <div className="flex items-center justify-between border-t border-brand-container-high pt-3">
                  <span className="font-sans text-xs font-semibold text-brand-text">Recalculated Lift-to-Drag Ratio</span>
                  <div className="flex items-center space-x-2">
                    <TrendingUp className="h-4.5 w-4.5 text-brand-primary animate-pulse" />
                    <span className="font-mono text-base font-bold text-brand-primary bg-white border border-brand-border rounded px-2.5 py-0.5 shadow-xs">
                      {drawerProject.parameters.liftDragRatio} Cl/Cd
                    </span>
                  </div>
                </div>
              </div>

              {/* Solver Convergence Data View */}
              <div>
                <div className="flex items-center justify-between">
                  <span className="font-mono text-[10px] font-bold text-brand-text-muted uppercase tracking-wider">Solver Convergence Performance</span>
                  <span className="font-sans text-[10px] text-brand-text-muted">Navier-Stokes steady state flow</span>
                </div>

                {/* Simulated Convergence Table Graph */}
                <div className="mt-2 rounded-lg border border-brand-border overflow-hidden bg-brand-bg p-3">
                  <div className="grid grid-cols-3 border-b border-brand-container-low px-2 py-1 font-mono text-[9px] text-brand-text-muted font-bold uppercase">
                    <span>Iteration</span>
                    <span>Lift Coef (Cl)</span>
                    <span>Drag Coef (Cd)</span>
                  </div>
                  <div className="max-h-36 overflow-y-auto divide-y divide-brand-bg font-mono text-xs text-brand-text pl-2">
                    {drawerProject.convergenceData.map((data, idx) => (
                      <div key={idx} className="grid grid-cols-3 py-1.5 hover:bg-white transition-colors">
                        <span className="font-bold text-brand-text-muted">#{data.iteration}</span>
                        <span>{data.lift}</span>
                        <span className="text-brand-accent">{data.drag}</span>
                      </div>
                    ))}
                  </div>

                  {isSolving ? (
                    <div className="mt-2.5 bg-white border border-brand-border p-2 rounded flex items-center justify-between animate-pulse">
                      <span className="font-sans text-[11px] font-bold text-brand-primary flex items-center gap-1.5">
                        <Cpu className="h-3.5 w-3.5 animate-spin" /> Mesh Solver running: {solveProgress}%
                      </span>
                      <div className="h-1.5 w-32 bg-brand-container rounded-full overflow-hidden">
                        <div className="h-full bg-brand-primary" style={{ width: `${solveProgress}%` }}></div>
                      </div>
                    </div>
                  ) : (
                    <div className="mt-2 flex justify-between items-center bg-white p-2 rounded border border-brand-bg">
                      <span className="font-sans text-[10px] text-brand-text-muted">Stability Index: <strong className="text-emerald-600 font-bold uppercase">Converged</strong></span>
                      <button 
                        onClick={() => handleDrawerSaveParams(drawerProject)}
                        className="rounded bg-brand-primary hover:bg-brand-accent px-2 py-1 text-[10px] font-bold text-white flex items-center gap-1"
                      >
                        <Play className="h-2.5 w-2.5" /> Force Resolve
                      </button>
                    </div>
                  )}
                </div>
              </div>

              {/* AI Recommendations Module */}
              <div className="space-y-2.5">
                <span className="font-mono text-[10px] font-bold text-brand-text-muted uppercase tracking-wider flex items-center gap-1">
                  <Sparkles className="h-3.5 w-3.5 text-brand-primary" /> Active AI Optimization Suggestions
                </span>

                <div className="space-y-2">
                  {drawerProject.suggestions.length === 0 ? (
                    <p className="font-sans text-xs text-brand-text-muted italic bg-brand-bg p-4 rounded text-center border">
                      No recommendation suggestions currently pending diagnostics optimization.
                    </p>
                  ) : (
                    drawerProject.suggestions.map((sug) => (
                      <div 
                        key={sug.id}
                        className={`border rounded-lg p-3 transition-all ${
                          sug.applied 
                            ? 'bg-emerald-50/20 border-emerald-200 opacity-80' 
                            : 'bg-white border-brand-border hover:border-brand-primary'
                        }`}
                      >
                        <div className="flex items-start justify-between">
                          <div>
                            <span className="font-mono text-[9px] uppercase font-semibold text-brand-primary bg-brand-container px-1 rounded">
                              {sug.category}
                            </span>
                            <h5 className="mt-1 font-sans text-xs font-bold text-brand-text">
                              {sug.title}
                            </h5>
                          </div>
                          
                          {sug.applied ? (
                            <span className="font-sans text-[10px] font-bold text-emerald-600 flex items-center gap-1">
                              <CheckCircle2 className="h-3.5 w-3.5" /> Applied
                            </span>
                          ) : (
                            <button
                              onClick={() => handleApplySuggestion(drawerProject, sug)}
                              className="rounded border border-brand-primary/40 bg-white hover:bg-brand-container px-2.5 py-1 text-[10px] font-bold text-brand-primary transition-colors hover:text-brand-accent"
                            >
                              Apply
                            </button>
                          )}
                        </div>
                        <p className="mt-1.5 font-sans text-[11px] text-brand-text-muted leading-relaxed">
                          {sug.description}
                        </p>
                        <div className="mt-2.5 flex items-center gap-1.5 text-[10px] font-bold font-mono text-emerald-600">
                          <TrendingUp className="h-3.5 w-3.5" /> EST. IMPACT: {sug.impact}
                        </div>
                      </div>
                    ))
                  )}
                </div>
              </div>

              {/* Dynamic Aerodynamic AI Assistant Sub-Drawer */}
              <div id="ai-chatbox-subdrawer" className="border-t border-brand-bg pt-4 space-y-3">
                <div className="flex items-center space-x-1.5">
                  <Terminal className="h-4.5 w-4.5 text-brand-primary" />
                  <span className="font-mono text-[10px] font-bold text-brand-text-muted uppercase tracking-wider">AeroFlow AI Assistant</span>
                </div>
                
                {/* Simulated Output logs */}
                <div className="max-h-48 overflow-y-auto space-y-2.5 rounded-lg border border-brand-border bg-brand-bg p-3 font-sans text-xs leading-relaxed">
                  <p className="text-brand-text-muted bg-white p-2 rounded border border-brand-container-low shadow-2xs">
                    💡 <strong>AeroFlow Bot:</strong> How can I assist you with specialized computational fluid dynamics optimization of <strong>{drawerProject.name}</strong> today? Try typing <em>"optimize wing"</em> or <em>"calculate mesh density"</em> below.
                  </p>
                  
                  {aiResponseList.map((chat, idx) => (
                    <div key={idx} className="space-y-1 ml-2">
                      <div className="text-right">
                        <span className="font-semibold bg-brand-primary text-white text-[10.5px] rounded-lg px-2.5 py-1 inline-block">
                          {chat.query}
                        </span>
                      </div>
                      <div className="text-left bg-white p-2.5 rounded-lg border border-brand-container-high text-brand-text">
                        <strong>AI Response:</strong> {chat.answer}
                      </div>
                    </div>
                  ))}
                </div>

                {/* Input query field */}
                <div className="flex items-center space-x-2">
                  <input
                    type="text"
                    placeholder="Ask AI aerodynamic questions about this project..."
                    value={customAIQuery}
                    onChange={(e) => setCustomAIQuery(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === 'Enter') runAerodynamicAIAdvisor();
                    }}
                    className="w-full rounded-md border border-brand-border bg-white px-3 py-1.5 font-sans text-xs text-brand-text outline-hidden"
                  />
                  <button
                    onClick={runAerodynamicAIAdvisor}
                    className="rounded-md bg-brand-primary px-3 py-1.5 font-sans text-xs font-bold text-white shadow-xs hover:bg-brand-accent h-8"
                  >
                    Submit
                  </button>
                </div>
              </div>
            </div>

            {/* Close footer */}
            <div className="border-t border-brand-bg bg-brand-bg px-6 py-3 flex justify-between items-center">
              <span className="font-mono text-[10px] text-brand-text-muted">Steady CFD Solver active</span>
              <div className="flex items-center space-x-2">
                <button 
                  onClick={() => {
                    props.onSelectProjectForDetails(drawerProject);
                    setDrawerProject(null);
                  }}
                  className="rounded bg-emerald-600 hover:bg-emerald-700 text-white px-4 py-1.5 font-sans text-xs font-bold shadow-xs flex items-center gap-1.5 cursor-pointer"
                >
                  <Activity className="h-3.5 w-3.5" /> Open in CFD Viewport
                </button>
                <button 
                  onClick={() => setDrawerProject(null)}
                  className="rounded border border-brand-border bg-white text-brand-text-muted hover:bg-brand-bg px-4 py-1.5 font-sans text-xs font-bold cursor-pointer"
                >
                  Close View
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Initialize New Project Modal Backdrop */}
      {showInitModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/45 backdrop-blur-xs p-4 animate-fade-in">
          <div className="w-full max-w-lg rounded-lg border border-brand-border bg-white p-6 shadow-2xl animate-slide-up">
            <div className="flex items-center justify-between border-b border-brand-bg pb-3">
              <h3 className="font-sans text-[13px] font-bold text-brand-primary uppercase tracking-wider flex items-center gap-1">
                <Layers className="h-4.5 w-4.5" /> Initialize New Aerodynamics Project
              </h3>
              <button onClick={() => setShowInitModal(false)} className="rounded-md p-1 hover:bg-brand-bg">
                <X className="h-4.5 w-4.5 text-brand-text-muted" />
              </button>
            </div>

            <form onSubmit={handleInitSubmit} className="mt-4 space-y-4">
              <div className="grid grid-cols-2 gap-3.5">
                <div>
                  <label className="block font-sans text-[11px] font-bold text-brand-text uppercase tracking-wide">Project Tag-Code Name</label>
                  <input
                    type="text"
                    required
                    placeholder="e.g. Project Delta-7"
                    value={initName}
                    onChange={(e) => setInitName(e.target.value)}
                    className="mt-1 w-full rounded-md border border-brand-border bg-brand-bg px-3 py-1.5 font-sans text-xs text-brand-text outline-hidden focus:border-brand-primary"
                  />
                </div>
                <div>
                  <label className="block font-sans text-[11px] font-bold text-brand-text uppercase tracking-wide">Initial Version ID</label>
                  <input
                    type="text"
                    placeholder="e.g. v1.0.0"
                    value={initVersion}
                    onChange={(e) => setInitVersion(e.target.value)}
                    className="mt-1 w-full rounded-md border border-brand-border bg-brand-bg px-3 py-1.5 font-sans text-xs text-brand-text outline-hidden focus:border-brand-primary"
                  />
                </div>
              </div>

              <div>
                <label className="block font-sans text-[11px] font-bold text-brand-text uppercase tracking-wide">Optimization description</label>
                <textarea
                  placeholder="Describe supersonic fluid simulation variables or aircraft geometry details..."
                  value={initDesc}
                  onChange={(e) => setInitDesc(e.target.value)}
                  className="mt-1 w-full h-16 rounded-md border border-brand-border bg-brand-bg px-3 py-1.5 font-sans text-xs text-brand-text outline-hidden focus:border-brand-primary"
                />
              </div>

              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block font-sans text-[11px] font-bold text-brand-text uppercase tracking-wide">Teammate initials (comma separated)</label>
                  <input
                    type="text"
                    value={initTeammates}
                    onChange={(e) => setInitTeammates(e.target.value)}
                    className="mt-1 w-full rounded-md border border-brand-border bg-brand-bg px-3 py-1.5 font-sans text-xs text-brand-text outline-hidden"
                  />
                </div>
                <div>
                  <label className="block font-sans text-[11px] font-bold text-brand-text uppercase tracking-wide">Boundary Status</label>
                  <select
                    value={initStatus}
                    onChange={(e) => setInitStatus(e.target.value as any)}
                    className="mt-1 w-full rounded-md border border-brand-border bg-brand-bg px-3 py-1.5 font-sans text-xs text-brand-text outline-hidden focus:ring-1 focus:ring-brand-primary"
                  >
                    <option value="active">Active (Multi-grid Dynamic)</option>
                    <option value="stable">Stable (Converged Boundary)</option>
                    <option value="archived">Archived (System Storage)</option>
                  </select>
                </div>
              </div>

              {/* CAD File Drag and Drop / Manual Select area */}
              <div className="space-y-1.5">
                <label className="block font-sans text-[11px] font-bold text-brand-text uppercase tracking-wide">
                  Optional 3D CAD Geometry Upload
                </label>
                
                {!uploadedCadFile ? (
                  <div
                    onDragOver={handleDragOver}
                    onDragLeave={handleDragLeave}
                    onDrop={handleDrop}
                    onClick={() => document.getElementById('cad-file-input')?.click()}
                    className={`flex flex-col items-center justify-center border-2 border-dashed rounded-lg p-5 transition-all duration-200 cursor-pointer text-center bg-brand-bg/30 ${
                      isDragging 
                        ? 'border-brand-primary bg-brand-container-low/20 scale-[0.99] border-solid' 
                        : 'border-brand-border hover:border-brand-primary hover:bg-white'
                    }`}
                  >
                    <input
                      id="cad-file-input"
                      type="file"
                      accept=".step,.stp,.iges,.igs,.stl,.obj,.f3d,.cad,.dwg,.dxf"
                      onChange={handleFileChange}
                      className="hidden"
                    />
                    <Upload className={`h-6 w-6 text-brand-text-muted mb-2 transition-transform ${isDragging ? 'translate-y-[-2px] text-brand-primary' : ''}`} />
                    <span className="font-sans text-xs font-bold text-brand-text">
                      Drag & drop your 3D CAD design file here, or <span className="text-brand-primary hover:underline">browse</span>
                    </span>
                    <p className="mt-1 font-sans text-[10px] text-brand-text-muted">
                      Supports STEP, STP, IGES, STL, OBJ, F3D up to 50MB
                    </p>
                  </div>
                ) : (
                  <div className="flex items-center justify-between border border-emerald-100 rounded-lg bg-emerald-50/10 p-3">
                    <div className="flex items-center space-x-2.5">
                      <div className="flex h-8 w-8 items-center justify-center rounded-md bg-emerald-50 text-emerald-600">
                        <File className="h-4.5 w-4.5" />
                      </div>
                      <div className="text-left">
                        <p className="font-sans text-xs font-bold text-brand-text truncate max-w-[220px]">
                          {uploadedCadFile.name}
                        </p>
                        <p className="font-mono text-[9.5px] text-emerald-600 font-semibold uppercase">
                          {uploadedCadFile.size} • format: {uploadedCadFile.type}
                        </p>
                      </div>
                    </div>
                    <button
                      type="button"
                      onClick={handleRemoveCadFile}
                      className="rounded p-1 text-red-500 hover:text-red-700 hover:bg-red-50 transition-all cursor-pointer"
                      title="Remove CAD file"
                    >
                      <Trash2 className="h-4 w-4" />
                    </button>
                  </div>
                )}
                
                {fileError && (
                  <p className="font-sans text-[10.5px] text-red-500 font-medium">
                    {fileError}
                  </p>
                )}
              </div>

              {/* Advanced Parameters Slider Init */}
              <div className="bg-brand-bg rounded-lg border border-brand-container-low p-4 space-y-3">
                <h4 className="font-mono text-[9px] font-bold uppercase tracking-wider text-brand-text">Initial Aerodynamic Airfoil Boundary Conditions</h4>
                <div className="grid grid-cols-2 gap-4">
                  <div>
                    <span className="flex justify-between font-sans text-[10.5px] text-brand-text-muted">Aspect Ratio: <strong className="font-mono">{initAspect}</strong></span>
                    <input type="range" min="1" max="15" step="0.1" value={initAspect} onChange={e => setInitAspect(parseFloat(e.target.value))} className="w-full accent-brand-primary" />
                  </div>
                  <div>
                    <span className="flex justify-between font-sans text-[10.5px] text-brand-text-muted">Reynolds Number: <strong className="font-mono">{initReynolds}M</strong></span>
                    <input type="range" min="1" max="30" step="0.5" value={initReynolds} onChange={e => setInitReynolds(parseFloat(e.target.value))} className="w-full accent-brand-primary" />
                  </div>
                  <div>
                    <span className="flex justify-between font-sans text-[10.5px] text-brand-text-muted">Sweep Angle: <strong className="font-mono">{initSweep}°</strong></span>
                    <input type="range" min="0" max="60" step="1" value={initSweep} onChange={e => setInitSweep(parseFloat(e.target.value))} className="w-full accent-brand-primary" />
                  </div>
                  <div>
                    <span className="flex justify-between font-sans text-[10.5px] text-brand-text-muted">Cruise Mach Speed: <strong className="font-mono">M {initMach}</strong></span>
                    <input type="range" min="0.1" max="2.5" step="0.02" value={initMach} onChange={e => setInitMach(parseFloat(e.target.value))} className="w-full accent-brand-primary" />
                  </div>
                </div>
              </div>

              <div className="mt-6 flex items-center justify-end space-x-3 border-t border-brand-bg pt-3">
                <button
                  type="button"
                  onClick={() => setShowInitModal(false)}
                  className="rounded-md border border-brand-border bg-white px-3.5 py-1.5 font-sans text-xs font-semibold text-brand-text-muted hover:bg-brand-bg focus:outline-hidden"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  className="rounded-md bg-brand-primary px-5 py-1.5 font-sans text-xs font-bold text-white shadow-xs hover:bg-brand-accent focus:outline-hidden"
                >
                  Initialize Solver
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}
