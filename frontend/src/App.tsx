import React, { useState } from 'react';
import { 
  initialUserProfile, 
  initialProjects, 
  initialLogs 
} from './data';
import { Project, UserProfile, SystemLog } from './types';
import Header from './components/Header';
import Sidebar from './components/Sidebar';
import ProfileSection from './components/ProfileSection';
import ActiveProjects from './components/ActiveProjects';
import AIPanel from './components/AIPanel';
import CFDSimulator from './components/CFDSimulator';
import DocGuide from './components/DocGuide';
import { 
  X, 
  Terminal, 
  FileSpreadsheet, 
  CheckCircle2, 
  Download,
  Database
} from 'lucide-react';

export default function App() {
  // Application Data States
  const [user, setUser] = useState<UserProfile>(initialUserProfile);
  const [projects, setProjects] = useState<Project[]>(initialProjects);
  const [logs, setLogs] = useState<SystemLog[]>(initialLogs);
  const [activeTab, setActiveTab] = useState<string>('projects');
  
  // Selected project active in CFD viewports
  const [selectedProjectForSolver, setSelectedProjectForSolver] = useState<Project | null>(initialProjects[0] || null);

  const handleSelectProjectForSolver = (project: Project | null) => {
    setSelectedProjectForSolver(project);
    setActiveTab('visualize');
  };
  
  // Interactive Plan Metric States (updated via Upgrades selection)
  const [planQuota, setPlanQuota] = useState({
    simsRemaining: 4,
    simsUsed: 1,
    tokensRemaining: 4050,
    tokensTotal: 5000,
    planName: 'PRO+'
  });

  // History system logs dialog toggle
  const [showLogsModal, setShowLogsModal] = useState(false);

  // Dynamic Suggestion Acceptance Rates (baseline 88.2% + incremental benefits when suggestions are applied)
  const calculateSuggestionAcceptance = () => {
    const historicalBase = 88.2;
    // Count applied suggestions across active projects
    let appliedCount = 0;
    projects.forEach((p) => {
      p.suggestions.forEach((s) => {
        if (s.applied) appliedCount += 1;
      });
    });
    return Math.min(100, historicalBase + (appliedCount * 2.9));
  };

  // State Updates Handlers
  const handleUpdateUser = (updated: UserProfile) => {
    setUser(updated);
  };

  const handleAddProject = (newProj: Project) => {
    // Add to project arrays and decrement plan limits!
    setProjects((prev) => [newProj, ...prev]);
    setPlanQuota((prev) => ({
      ...prev,
      simsRemaining: Math.max(0, prev.simsRemaining - 1),
      simsUsed: prev.simsUsed + 1,
      tokensRemaining: Math.max(0, prev.tokensRemaining - 200) // Deduct mock solver costs
    }));

    // Append to system logs tracing
    const newLog: SystemLog = {
      id: `log-init-${Date.now()}`,
      timestamp: new Date().toISOString().replace('T', ' ').slice(0, 19),
      project: newProj.name,
      event: `Aerodynamic grid build and boundary configuration standard solver ${newProj.version}`,
      status: 'SUCCESS',
      computeCost: 200
    };
    setLogs((prev) => [newLog, ...prev]);
  };

  const handleUpdateProject = (updatedProj: Project) => {
    setProjects((prev) => prev.map((p) => p.id === updatedProj.id ? updatedProj : p));
  };

  const handleApplyAISuggestion = (projId: string, suggestionId: string) => {
    setProjects((prev) => 
      prev.map((proj) => {
        if (proj.id !== projId) return proj;
        
        // Find is matching suggestion and apply
        const suggestions = proj.suggestions.map((sug) => {
          if (sug.id === suggestionId) {
            return { ...sug, applied: true };
          }
          return sug;
        });

        // Deduct compute limit and log
        setPlanQuota((prevQuota) => ({
          ...prevQuota,
          tokensRemaining: Math.max(0, prevQuota.tokensRemaining - 85)
        }));

        const sName = proj.suggestions.find(s => s.id === suggestionId)?.title || 'Airfoil sweep boundary';
        const newLog: SystemLog = {
          id: `log-sug-${Date.now()}`,
          timestamp: new Date().toISOString().replace('T', ' ').slice(0, 19),
          project: proj.name,
          event: `Applied AI refinement: ${sName}`,
          status: 'CONVERGED',
          computeCost: 85
        };
        setLogs((prevLogs) => [newLog, ...prevLogs]);

        return { ...proj, suggestions };
      })
    );
  };

  const handleUpdatePlan = (newQuota: { simsRemaining: number; simsUsed: number; tokensRemaining: number; tokensTotal: number; planName: string }) => {
    setPlanQuota(newQuota);
    
    // Add a log mapping package upgrade
    const newLog: SystemLog = {
      id: `log-plan-${Date.now()}`,
      timestamp: new Date().toISOString().replace('T', ' ').slice(0, 19),
      project: 'AeroFlow Cluster',
      event: `Upgraded server resource plan to: ${newQuota.planName}`,
      status: 'SUCCESS',
      computeCost: 0
    };
    setLogs((prev) => [newLog, ...prev]);
  };

  // Filter projects based on left navigation scope
  const getFilteredProjects = () => {
    if (activeTab === 'my-projects') {
      return projects.filter(p => p.status === 'active' || p.status === 'stable');
    }
    if (activeTab === 'shared') {
      // Return a simulated shared list (e.g., Wing Assembly v4)
      return projects.filter(p => p.id === 'wing-assembly-v4');
    }
    if (activeTab === 'public') {
      // Standard public datasets
      return projects.filter(p => p.status === 'stable' || p.status === 'archived');
    }
    // Default or recent shows all in descending order
    return projects;
  };

  return (
    <div id="aeroflow-root" className="min-h-screen bg-brand-bg flex flex-col text-brand-text">
      {/* 1. Header component */}
      <Header 
        user={user} 
        onChangeUser={handleUpdateUser}
        onNavigate={setActiveTab}
        activeTab={activeTab}
      />

      <div className="flex-1 flex flex-col md:flex-row">
        {/* 2. Left side navigation */}
        <Sidebar 
          activeTab={activeTab} 
          onNavigate={setActiveTab} 
          planQuota={planQuota}
          onUpdatePlan={handleUpdatePlan}
        />

        {/* 3. Main content arena */}
        <main className="flex-1 p-6 overflow-y-auto max-w-7xl mx-auto w-full">
          {activeTab === 'visualize' ? (
            // Full breadth CFD sandbox simulator
            <CFDSimulator 
              activeProject={selectedProjectForSolver}
              allProjects={projects}
              onSelectProject={setSelectedProjectForSolver}
            />
          ) : activeTab === 'doc' ? (
            // Formulations and manuals guides
            <DocGuide />
          ) : (
            // Standard Dashboards columns (Middle and right matching mockup layout)
            <div className="grid grid-cols-1 gap-6 lg:grid-cols-12">
              {/* Middle Section Dashboard Panels - 8 Columns */}
              <div className="space-y-6 lg:col-span-8">
                {/* Profile Card Summary */}
                <ProfileSection 
                  user={user} 
                  onUpdateUser={handleUpdateUser} 
                />

                {/* Grid list of active simulations */}
                <ActiveProjects 
                  projects={getFilteredProjects()} 
                  onAddProject={handleAddProject}
                  onUpdateProject={handleUpdateProject}
                  onApplyAISuggestion={handleApplyAISuggestion}
                  onSelectProjectForDetails={handleSelectProjectForSolver}
                />
              </div>

              {/* Right Section Metrics Columns - 4 Columns */}
              <div className="space-y-6 lg:col-span-4">
                <AIPanel 
                  suggestionAcceptanceRate={calculateSuggestionAcceptance()} 
                  logs={logs}
                  onShowFullLogs={() => setShowLogsModal(true)}
                />
              </div>
            </div>
          )}
        </main>
      </div>

      {/* 4. Full Navier-Stokes Computations History trace logs modal dialog */}
      {showLogsModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/45 backdrop-blur-xs p-4 animate-fade-in animate-fade-in">
          <div className="w-full max-w-3xl rounded-lg border border-brand-border bg-white p-5 shadow-2xl flex flex-col h-[520px] justify-between animate-slide-up">
            {/* Header */}
            <div className="flex items-center justify-between border-b pb-3 mb-3">
              <div className="flex items-center space-x-2 text-brand-primary">
                <Database className="h-5 w-5" />
                <h3 className="font-sans text-xs font-bold uppercase tracking-wider text-brand-text">Navier-Stokes Solvers Diagnostics History</h3>
              </div>
              <button onClick={() => setShowLogsModal(false)} className="rounded p-1 hover:bg-brand-bg md:p-1.5">
                <X className="h-4.5 w-4.5 text-brand-text-muted" />
              </button>
            </div>

            {/* Scrollable table grid logs */}
            <div className="flex-1 overflow-y-auto min-h-0 space-y-3">
              <div className="border rounded-md overflow-hidden bg-brand-bg">
                <div className="grid grid-cols-12 bg-white px-3 py-2 font-mono text-[9px] text-brand-text-muted border-b font-bold uppercase tracking-wider">
                  <span className="col-span-3">Timestamp / Node</span>
                  <span className="col-span-3">Project Domain</span>
                  <span className="col-span-4">Operation Solver Thread</span>
                  <span className="col-span-2 text-right">Convergence</span>
                </div>

                <div className="divide-y divide-brand-container-low font-mono text-xs pl-3">
                  {logs.map((item) => (
                    <div key={item.id} className="grid grid-cols-12 py-2.5 items-center pr-3 hover:bg-white transition-colors">
                      <span className="col-span-3 text-brand-text-muted text-[10.5px]">{item.timestamp}</span>
                      <span className="col-span-3 font-semibold text-brand-text">{item.project}</span>
                      <span className="col-span-4 text-[11px] text-brand-text-muted truncate pr-2">{item.event}</span>
                      <span className="col-span-2 text-right">
                        <span className={`px-1.5 py-0.5 rounded text-[9.5px] font-bold uppercase ${
                          item.status === 'CONVERGED'
                            ? 'bg-emerald-50 text-emerald-700 border border-emerald-100'
                            : item.status === 'RUNNING'
                            ? 'bg-amber-55 bg-amber-50 text-amber-700 animate-pulse'
                            : 'bg-brand-container text-brand-primary'
                        }`}>
                          {item.status}
                        </span>
                      </span>
                    </div>
                  ))}
                </div>
              </div>

              {/* simulated running terminal tracer */}
              <div className="rounded bg-slate-900 p-3 font-mono text-[10.5px] text-slate-100 space-y-1.5 shadow-inner">
                <div className="flex items-center space-x-2 text-[9.5px] text-brand-accent uppercase font-bold tracking-wider border-b border-slate-800 pb-1.5">
                  <Terminal className="h-3.5 w-3.5" />
                  <span>Interactive Telemetry Cluster Stdout stream</span>
                </div>
                <code>[SYS] Connecting to Bern Research cloud cluster... established on Node-8</code>
                <code>[GRID] 12,485,380 grid meshes mapped in laminar buffer stream</code>
                <code>[SOLVE] SST K-Omega turbulence model active. Cl={projects[0]?.parameters.liftDragRatio ? (projects[0].parameters.liftDragRatio * 0.04).toFixed(3) : '0.655'} Cd={projects[0]?.parameters.liftDragRatio ? (0.655 / projects[0].parameters.liftDragRatio).toFixed(4) : '0.030'}</code>
                <code>[INFO] Residual criteria met below 1.0e-6 threshold. Multi-grid cache compiled successfully.</code>
              </div>
            </div>

            {/* Footer triggers */}
            <div className="border-t pt-3 mt-3 flex items-center justify-between">
              <span className="font-mono text-[10px] text-brand-text-muted">Database logs verified • SHA256 Secure</span>
              <button
                onClick={() => setShowLogsModal(false)}
                className="rounded bg-brand-primary hover:bg-brand-accent text-white px-4 py-1.5 font-sans text-xs font-bold shadow-xs"
              >
                Done
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
