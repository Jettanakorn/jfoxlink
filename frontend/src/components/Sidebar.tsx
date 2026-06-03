import React, { useState } from 'react';
import { 
  FolderGit2, 
  Hourglass, 
  Sparkles,
  Layers, 
  History, 
  Users, 
  FolderClosed, 
  Globe2, 
  Cpu, 
  LineChart, 
  ArrowUpCircle, 
  BookOpen,
  CheckCircle2,
  X,
  CreditCard
} from 'lucide-react';

interface SidebarProps {
  activeTab: string;
  onNavigate: (tab: string) => void;
  planQuota: {
    simsRemaining: number;
    simsUsed: number;
    tokensRemaining: number;
    tokensTotal: number;
  };
  onUpdatePlan: (quota: { simsRemaining: number; simsUsed: number; tokensRemaining: number; tokensTotal: number; planName: string }) => void;
}

export default function Sidebar(props: SidebarProps) {
  const [showUpgradeModal, setShowUpgradeModal] = useState(false);
  const [selectedPlanUpgrade, setSelectedPlanUpgrade] = useState<'pro' | 'research' | 'ultimate'>('research');
  const [planName, setPlanName] = useState('PRO+');

  const navigationItems = [
    { id: 'projects', label: 'Recent Projects', icon: Layers },
    { id: 'my-projects', label: 'My Projects', icon: FolderClosed },
    { id: 'shared', label: 'Share with Me', icon: Users },
    { id: 'public', label: 'Public Projects', icon: Globe2 },
    { id: 'visualize', label: 'Visualize CFD Flow', icon: LineChart },
    { id: 'doc', label: 'Documentation', icon: BookOpen },
  ];

  const triggerPlanSelect = (plan: 'pro' | 'research' | 'ultimate') => {
    setSelectedPlanUpgrade(plan);
    if (plan === 'pro') {
      props.onUpdatePlan({ simsRemaining: 4, simsUsed: 1, tokensRemaining: 4050, tokensTotal: 5000, planName: 'PRO+' });
      setPlanName('PRO+');
    } else if (plan === 'research') {
      props.onUpdatePlan({ simsRemaining: 15, simsUsed: 5, tokensRemaining: 12500, tokensTotal: 15000, planName: 'RESEARCH MULTI-NODE' });
      setPlanName('RESEARCH MULTI-NODE');
    } else {
      props.onUpdatePlan({ simsRemaining: 100, simsUsed: 0, tokensRemaining: 500000, tokensTotal: 500000, planName: 'ULTIMATE ENTERPRISE' });
      setPlanName('ULTIMATE ENTERPRISE');
    }
  };

  const simPercent = (props.planQuota.simsUsed / (props.planQuota.simsRemaining + props.planQuota.simsUsed)) * 100;
  const tokenPercent = ((props.planQuota.tokensTotal - props.planQuota.tokensRemaining) / props.planQuota.tokensTotal) * 100;

  return (
    <aside id="aeroflow-sidebar" className="flex h-[calc(100vh-61px)] w-64 flex-col justify-between border-r border-brand-border bg-white p-4">
      {/* Upper Navigation Sections */}
      <div className="space-y-6">
        {/* Active Workspace / Plan Context */}
        <div className="px-2">
          <div className="flex items-center space-x-2">
            <FolderGit2 className="h-4 w-4 text-brand-primary" />
            <span className="font-sans text-xs font-bold uppercase tracking-wider text-brand-text">Projects</span>
          </div>
          <div className="mt-1 flex items-center space-x-1">
            <span className="font-mono text-[10px] uppercase font-bold text-brand-text-muted">AeroFlow Plan:</span>
            <span className="font-mono text-[10px] font-bold text-brand-primary">{planName}</span>
          </div>
        </div>

        {/* Dynamic Nav link Items */}
        <nav className="space-y-1">
          {navigationItems.map((item) => {
            const Icon = item.icon;
            const isActive = props.activeTab === item.id;
            return (
              <button
                key={item.id}
                onClick={() => props.onNavigate(item.id)}
                className={`flex w-full items-center space-x-3 rounded-md px-3 py-2 text-left font-sans text-xs font-semibold tracking-tight transition-all ${
                  isActive 
                    ? 'bg-brand-container-high text-brand-primary' 
                    : 'text-brand-text-muted hover:bg-brand-container-low hover:text-brand-text'
                }`}
              >
                <Icon className={`h-4 w-4 ${isActive ? 'text-brand-primary' : 'text-brand-text-muted'}`} />
                <span>{item.label}</span>
              </button>
            );
          })}
        </nav>

        {/* Diagnostic compute and simulation metrics indicators as seen on original layout */}
        <div className="space-y-4 rounded-lg bg-brand-bg p-3 border border-brand-container-low">
          {/* Simulation Quota Tracker */}
          <div>
            <div className="flex items-center justify-between text-[11px] font-semibold text-brand-text">
              <span className="font-sans font-bold uppercase tracking-wider text-[10px] text-brand-text-muted">Simulations Remaining</span>
            </div>
            <div className="mt-1 font-sans text-xs font-bold text-brand-text">
              {props.planQuota.simsRemaining} remaining <span className="text-[10px] font-normal text-brand-text-muted">({props.planQuota.simsUsed} of {props.planQuota.simsRemaining + props.planQuota.simsUsed} used)</span>
            </div>
            <div className="mt-1.5 h-1.5 w-full overflow-hidden rounded-full bg-brand-container-high">
              <div 
                className="h-full rounded-full bg-brand-primary transition-all duration-500"
                style={{ width: `${simPercent}%` }}
              ></div>
            </div>
          </div>

          {/* Compute Limit Counter */}
          <div>
            <div className="flex items-center justify-between text-[11px] font-semibold text-brand-text">
              <span className="font-sans font-bold uppercase tracking-wider text-[10px] text-brand-text-muted">Tokens Compute Usage</span>
            </div>
            <div className="mt-1 font-sans text-xs font-bold text-brand-text">
              {props.planQuota.tokensRemaining.toLocaleString()} tokens left
            </div>
            <p className="mt-0.5 font-sans text-[10px] text-brand-text-muted leading-tight">
              ({(props.planQuota.tokensTotal - props.planQuota.tokensRemaining).toLocaleString()} of {props.planQuota.tokensTotal.toLocaleString()} tokens used)
            </p>
            <div className="mt-1.5 h-1.5 w-full overflow-hidden rounded-full bg-brand-container-high">
              <div 
                className="h-full rounded-full bg-brand-accent transition-all duration-500"
                style={{ width: `${tokenPercent}%` }}
              ></div>
            </div>
          </div>
        </div>
      </div>

      {/* Footer Navigation section */}
      <div className="space-y-2">
        <button
          onClick={() => setShowUpgradeModal(true)}
          className="flex w-full items-center justify-center space-x-2 rounded-md bg-brand-primary px-3 py-2.5 text-center font-sans text-xs font-bold text-white shadow-xs transition-colors hover:bg-brand-accent"
        >
          <ArrowUpCircle className="h-4 w-4" />
          <span>Upgrade Your Plan</span>
        </button>

        {/* Documentation fast-access */}
        <button
          onClick={() => props.onNavigate('doc')}
          className="flex w-full items-center justify-center space-x-1 rounded-md border border-brand-border bg-white py-2 text-center font-sans text-xs font-semibold text-brand-text-muted transition-colors hover:bg-brand-bg hover:text-brand-text"
        >
          <BookOpen className="h-4 w-4" />
          <span>Developer Docs</span>
        </button>

        {/* Compact copyright line */}
        <div className="pt-2 text-center">
          <span className="font-mono text-[9px] text-brand-text-muted">AeroFlow v2.8.4 • Cloud Powered</span>
        </div>
      </div>

      {/* Upgrade Plan Modal */}
      {showUpgradeModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-xs p-4 animate-fade-in">
          <div className="w-full max-w-md rounded-lg border border-brand-border bg-white p-5 shadow-2xl animate-slide-up">
            <div className="flex items-center justify-between border-b border-brand-bg pb-3">
              <div className="flex items-center space-x-2">
                <Sparkles className="h-5 w-5 text-brand-primary" />
                <h3 className="font-sans text-sm font-bold text-brand-text">Select Compute Cluster Plan</h3>
              </div>
              <button 
                onClick={() => setShowUpgradeModal(false)}
                className="rounded-md p-1 hover:bg-brand-bg"
              >
                <X className="h-4 w-4 text-brand-text-muted" />
              </button>
            </div>

            {/* Plan cards */}
            <div className="mt-4 space-y-3">
              {/* Pro Plan */}
              <div 
                onClick={() => triggerPlanSelect('pro')}
                className={`relative cursor-pointer rounded-lg border p-3 transition-all ${selectedPlanUpgrade === 'pro' ? 'border-brand-primary bg-brand-container-low/50 ring-1 ring-brand-primary' : 'border-brand-border hover:bg-brand-bg'}`}
              >
                <div className="flex items-center justify-between">
                  <span className="font-sans text-xs font-bold text-brand-text">AeroFlow PRO+</span>
                  <span className="font-mono text-xs font-bold text-brand-primary">$49/mo</span>
                </div>
                <p className="mt-1 font-sans text-[11px] text-brand-text-muted">Ideal for individual aerodynamics researchers. 5 total sims & 5K computes.</p>
                {selectedPlanUpgrade === 'pro' && <CheckCircle2 className="absolute right-2.5 bottom-2.5 h-4 w-4 text-brand-primary" />}
              </div>

              {/* Research Multi-Node Plan */}
              <div 
                onClick={() => triggerPlanSelect('research')}
                className={`relative cursor-pointer rounded-lg border p-3 transition-all ${selectedPlanUpgrade === 'research' ? 'border-brand-primary bg-brand-container-low/50 ring-1 ring-brand-primary' : 'border-brand-border hover:bg-brand-bg'}`}
              >
                <div className="flex items-center justify-between">
                  <span className="font-sans text-xs font-bold text-brand-text flex items-center gap-1">Research Multi-Node <span className="bg-brand-container font-mono text-[9px] px-1 py-0.5 rounded text-brand-primary uppercase">Best Value</span></span>
                  <span className="font-mono text-xs font-bold text-brand-primary">$189/mo</span>
                </div>
                <p className="mt-1 font-sans text-[11px] text-brand-text-muted">Parallel multicore iterations. 20 simulations quota, 15K token compute.</p>
                {selectedPlanUpgrade === 'research' && <CheckCircle2 className="absolute right-2.5 bottom-2.5 h-4 w-4 text-brand-primary" />}
              </div>

              {/* Ultimate Enterprise Plan */}
              <div 
                onClick={() => triggerPlanSelect('ultimate')}
                className={`relative cursor-pointer rounded-lg border p-3 transition-all ${selectedPlanUpgrade === 'ultimate' ? 'border-brand-primary bg-brand-container-low/50 ring-1 ring-brand-primary' : 'border-brand-border hover:bg-brand-bg'}`}
              >
                <div className="flex items-center justify-between">
                  <span className="font-sans text-xs font-bold text-brand-text">Ultimate Enterprise</span>
                  <span className="font-mono text-xs font-bold text-brand-primary">$899/mo</span>
                </div>
                <p className="mt-1 font-sans text-[11px] text-brand-text-muted">Dedicated server shards. 100 simulations quota & unlimited 500K compute token.</p>
                {selectedPlanUpgrade === 'ultimate' && <CheckCircle2 className="absolute right-2.5 bottom-2.5 h-4 w-4 text-brand-primary" />}
              </div>
            </div>

            <div className="mt-5 flex items-center justify-between space-x-3">
              <div className="flex items-center text-xs text-brand-text-muted">
                <CreditCard className="mr-1.5 h-4 w-4" />
                <span>Billed monthly via card</span>
              </div>
              <button
                onClick={() => setShowUpgradeModal(false)}
                className="rounded-md bg-brand-primary px-4 py-1.5 font-sans text-xs font-bold text-white shadow-xs hover:bg-brand-accent"
              >
                Apply Upgrade
              </button>
            </div>
          </div>
        </div>
      )}
    </aside>
  );
}
