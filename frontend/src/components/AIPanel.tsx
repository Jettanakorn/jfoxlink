import React, { useState } from 'react';
import { 
  Zap, 
  Cpu, 
  CheckCircle2, 
  Lightbulb, 
  ChevronRight, 
  CreditCard, 
  ShieldCheck, 
  Coins, 
  X, 
  BarChart2, 
  Printer, 
  ArrowDownToLine, 
  Clock, 
  Terminal,
  FileSpreadsheet
} from 'lucide-react';
import { SystemLog } from '../types';

interface AIPanelProps {
  suggestionAcceptanceRate: number;
  logs: SystemLog[];
  onShowFullLogs: () => void;
}

export default function AIPanel(props: AIPanelProps) {
  const [activeSheet, setActiveSheet] = useState<'billing' | 'privacy' | 'earning' | null>(null);
  const [hasSharedCluster, setHasSharedCluster] = useState(false);
  const [clusterStrength, setClusterStrength] = useState(75);
  const [billingLimit, setBillingLimit] = useState(2500);

  // Download simulation records as a JSON file
  const triggerLogExport = () => {
    const dataStr = "data:text/json;charset=utf-8," + encodeURIComponent(JSON.stringify(props.logs, null, 2));
    const downloadAnchor = document.createElement('a');
    downloadAnchor.setAttribute("href", dataStr);
    downloadAnchor.setAttribute("download", "aeroflow_navier_stokes_logs.json");
    document.body.appendChild(downloadAnchor);
    downloadAnchor.click();
    downloadAnchor.remove();
  };

  return (
    <div id="ai-panel-container" className="space-y-6">
      {/* Header and Label */}
      <div className="flex items-center space-x-2">
        <Zap className="h-4.5 w-4.5 text-brand-accent animate-pulse" />
        <div>
          <h3 className="font-sans text-sm font-bold uppercase tracking-wider text-brand-text flex items-center gap-1.5">
            AI Insights
          </h3>
          <span className="font-mono text-[9px] text-brand-text-muted uppercase tracking-wider block leading-none">
            Autonomous Diagnostics
          </span>
        </div>
      </div>

      {/* Metrics Widgets */}
      <div className="space-y-4">
        {/* Metric 1: Compute Hours */}
        <div className="rounded-lg border border-brand-border bg-white p-4">
          <div className="flex items-start justify-between">
            <div>
              <span className="font-sans text-[11px] font-bold text-brand-text-muted uppercase tracking-wide">
                Compute Hours Consumed
              </span>
              <div className="mt-1 font-mono text-2xl font-bold text-brand-text">
                1,248.5 <span className="font-sans text-xs font-normal text-brand-text-muted">hrs</span>
              </div>
            </div>
            <div className="rounded-md bg-brand-container p-1 text-brand-primary">
              <Cpu className="h-4.5 w-4.5" />
            </div>
          </div>

          <div className="mt-3">
            <div className="flex justify-between font-sans text-[10px] text-brand-text-muted">
              <span>Monthly quota: 1,600 hrs</span>
              <span className="font-bold font-mono text-brand-primary">78%</span>
            </div>
            <div className="mt-1.5 h-1.5 w-full overflow-hidden rounded-full bg-brand-container-high">
              <div className="h-full rounded-full bg-brand-primary transition-all duration-500" style={{ width: '78%' }}></div>
            </div>
          </div>
        </div>

        {/* Metric 2: Sim Success Rate */}
        <div className="rounded-lg border border-brand-border bg-white p-4">
          <div className="flex items-start justify-between">
            <div>
              <span className="font-sans text-[11px] font-bold text-brand-text-muted uppercase tracking-wide">
                Sim Success Rate
              </span>
              <div className="mt-1 font-mono text-2xl font-bold text-brand-text">
                96.4%
              </div>
            </div>
            <div className="rounded-md bg-emerald-50 p-1 text-emerald-600 border border-emerald-100">
              <CheckCircle2 className="h-4.5 w-4.5" />
            </div>
          </div>
          <span className="mt-2.5 inline-block font-mono text-[10.5px] font-bold text-emerald-600">
            ↑ +2.1% from last month
          </span>
        </div>

        {/* Metric 3: AI Suggestion Acceptance */}
        <div className="rounded-lg border border-brand-border bg-white p-4">
          <div className="flex items-start justify-between">
            <div>
              <span className="font-sans text-[11px] font-bold text-brand-text-muted uppercase tracking-wide">
                AI Suggestion Acceptance
              </span>
              <div className="mt-1 font-mono text-2xl font-bold text-brand-text">
                {props.suggestionAcceptanceRate.toFixed(1)}%
              </div>
            </div>
            <div className="rounded-md bg-brand-container p-1 text-brand-primary">
              <Lightbulb className="h-4.5 w-4.5 text-brand-accent h-4.5 w-4.5" />
            </div>
          </div>
          <p className="mt-2.5 font-sans text-xs text-brand-text-muted leading-relaxed">
            Your feedback has refined 42 mesh optimization protocols.
          </p>
        </div>
      </div>

      {/* Quick Action Navigation items matching the buttons list */}
      <div className="space-y-2 border-t border-brand-bg pt-4">
        {/* Billing Information button */}
        <button
          onClick={() => setActiveSheet('billing')}
          className="flex w-full items-center justify-between rounded-md border border-brand-border bg-white px-3 py-2.5 tracking-tight font-sans text-xs font-semibold text-brand-text hover:bg-brand-container-low/50 transition-colors"
        >
          <span className="flex items-center gap-2"><CreditCard className="h-4 w-4 text-brand-text-muted" /> Billing Information</span>
          <ChevronRight className="h-4 w-4 text-brand-text-muted" />
        </button>

        {/* Privacy & Security button */}
        <button
          onClick={() => setActiveSheet('privacy')}
          className="flex w-full items-center justify-between rounded-md border border-brand-border bg-white px-3 py-2.5 tracking-tight font-sans text-xs font-semibold text-brand-text hover:bg-brand-container-low/50 transition-colors"
        >
          <span className="flex items-center gap-2"><ShieldCheck className="h-4 w-4 text-brand-text-muted" /> Privacy & Security</span>
          <ChevronRight className="h-4 w-4 text-brand-text-muted" />
        </button>

        {/* Earning Program button */}
        <button
          onClick={() => setActiveSheet('earning')}
          className="flex w-full items-center justify-between rounded-md border border-brand-border bg-white px-3 py-2.5 tracking-tight font-sans text-xs font-semibold text-brand-text hover:bg-brand-container-low/50 transition-colors"
        >
          <span className="flex items-center gap-2"><Coins className="h-4 w-4 text-brand-text-muted" /> Earning Program</span>
          <ChevronRight className="h-4 w-4 text-brand-text-muted" />
        </button>
      </div>

      {/* History and Export actions */}
      <div className="flex items-center justify-between border-t border-brand-bg pt-4">
        <button 
          onClick={props.onShowFullLogs}
          className="flex items-center space-x-1 font-sans text-xs font-semibold text-brand-text-muted hover:text-brand-primary transition-colors cursor-pointer"
        >
          <Clock className="h-4 w-4" />
          <span>History</span>
        </button>
        <button 
          onClick={triggerLogExport}
          className="flex items-center space-x-1 font-sans text-xs font-semibold text-brand-text-muted hover:text-brand-primary transition-colors cursor-pointer"
        >
          <ArrowDownToLine className="h-4 w-4" />
          <span>Export Logs</span>
        </button>
      </div>

      {/* Interstitial Sliding Sheets */}
      {activeSheet && (
        <div className="fixed inset-0 z-50 flex items-center justify-end bg-black/40 backdrop-blur-xs p-4 animate-fade-in">
          <div className="h-full w-full max-w-lg rounded-lg border border-brand-border bg-white p-5 shadow-2xl flex flex-col justify-between animate-slide-in">
            {/* Sheet Header */}
            <div className="flex items-center justify-between border-b pb-3 mb-4">
              <div className="flex items-center space-x-2">
                {activeSheet === 'billing' && <CreditCard className="h-5 w-5 text-brand-primary" />}
                {activeSheet === 'privacy' && <ShieldCheck className="h-5 w-5 text-brand-primary" />}
                {activeSheet === 'earning' && <Coins className="h-5 w-5 text-brand-primary" />}
                <h3 className="font-sans text-xs font-bold uppercase tracking-wider text-brand-text">
                  {activeSheet === 'billing' && 'Billing Ledger & Invoices'}
                  {activeSheet === 'privacy' && 'Privacy, Encryption & Hardware Keys'}
                  {activeSheet === 'earning' && 'AeroFlow Research Earning Matrix'}
                </h3>
              </div>
              <button onClick={() => setActiveSheet(null)} className="rounded-md p-1 hover:bg-brand-bg">
                <X className="h-4.5 w-4.5" />
              </button>
            </div>

            {/* Sheet Content Body */}
            <div className="flex-1 overflow-y-auto space-y-5 text-xs text-brand-text">
              {/* BILLING MODULE */}
              {activeSheet === 'billing' && (
                <div className="space-y-4">
                  <div className="rounded-lg bg-brand-bg border p-3.5 space-y-2">
                    <span className="font-sans font-bold text-[10px] uppercase tracking-wide text-brand-text-muted">Estimated Spend This Month</span>
                    <div className="flex items-baseline justify-between">
                      <span className="font-mono text-lg font-bold text-brand-text">$485.50 / $1,200 quota</span>
                      <span className="text-[10px] text-brand-text-muted">Next invoice June 1, 2026</span>
                    </div>
                    {/* Quota bar */}
                    <div className="h-1.5 w-full bg-brand-container border rounded-full overflow-hidden">
                      <div className="h-full bg-brand-accent" style={{ width: '40%' }}></div>
                    </div>
                  </div>

                  {/* Monthly billing bar chart simulator */}
                  <div className="space-y-2">
                    <span className="font-mono text-[9px] font-bold text-brand-text-muted uppercase">Usage History (Past 4 Months)</span>
                    <div className="flex h-32 items-end justify-between border-b border-brand-border px-4 py-2 bg-brand-bg rounded-md">
                      <div className="flex flex-col items-center">
                        <div className="w-8 rounded-t-sm bg-brand-container-high h-12"></div>
                        <span className="mt-1 font-mono text-[9px] text-brand-text-muted">Jan</span>
                      </div>
                      <div className="flex flex-col items-center">
                        <div className="w-8 rounded-t-sm bg-brand-container-high h-20"></div>
                        <span className="mt-1 font-mono text-[9px] text-brand-text-muted">Feb</span>
                      </div>
                      <div className="flex flex-col items-center">
                        <div className="w-8 rounded-t-sm bg-brand-container-high h-24"></div>
                        <span className="mt-1 font-mono text-[9px] text-brand-text-muted">Mar</span>
                      </div>
                      <div className="flex flex-col items-center">
                        <div className="w-8 rounded-t-sm bg-brand-primary h-28"></div>
                        <span className="mt-1 font-mono text-[9px] text-brand-text-muted">Apr (Active)</span>
                      </div>
                    </div>
                  </div>

                  {/* PDF Invoices Table */}
                  <div className="space-y-2">
                    <span className="font-mono text-[9px] font-bold text-brand-text-muted uppercase">Past Invoices CSV/PDF</span>
                    <div className="border rounded-md divide-y overflow-hidden">
                      <div className="flex justify-between p-2 hover:bg-brand-bg">
                        <span>Invoice #INV-2026-04</span>
                        <button className="text-brand-primary hover:underline font-bold flex items-center gap-0.5"><Printer className="h-3 w-3" /> PDF</button>
                      </div>
                      <div className="flex justify-between p-2 hover:bg-brand-bg">
                        <span>Invoice #INV-2026-03</span>
                        <button className="text-brand-primary hover:underline font-bold flex items-center gap-0.5"><Printer className="h-3 w-3" /> PDF</button>
                      </div>
                    </div>
                  </div>
                </div>
              )}

              {/* PRIVACY & SECURITY MODULE */}
              {activeSheet === 'privacy' && (
                <div className="space-y-4">
                  <div className="space-y-1 bg-brand-bg border p-3 rounded-md">
                    <h4 className="font-sans font-bold text-xs">Cryptographic Node Signing</h4>
                    <p className="text-[11px] text-brand-text-muted leading-relaxed">
                      All cluster simulations and Navier-Stokes grid models are encrypted using AES-256 GCM hardware signatures at your Bern Research Facility node scope.
                    </p>
                  </div>

                  {/* Access Token display keys */}
                  <div className="space-y-2">
                    <span className="font-mono text-[9px] font-bold text-brand-text-muted uppercase">Active Solver SSH API Access Token</span>
                    <div className="p-2.5 rounded border bg-brand-bg font-mono text-[10.5px] text-brand-text break-all flex items-center justify-between">
                      <code>aes_af_prod_token_8827c19bbba23efcd3...</code>
                      <span className="text-emerald-600 font-bold ml-2">Active</span>
                    </div>
                  </div>

                  {/* Data retention toggles */}
                  <div className="space-y-2 border-t pt-2">
                    <span className="font-mono text-[9px] font-bold text-brand-text-muted uppercase">Solvers Data Retention</span>
                    <div className="space-y-2.5 mt-1.5">
                      <div className="flex justify-between items-center bg-brand-bg p-2 rounded">
                        <span>Purge solved grid cache after 30 days</span>
                        <input type="checkbox" defaultChecked className="accent-brand-primary" />
                      </div>
                      <div className="flex justify-between items-center bg-brand-bg p-2 rounded">
                        <span>Export CSV matrices as raw binary logs</span>
                        <input type="checkbox" className="accent-brand-primary" />
                      </div>
                    </div>
                  </div>
                </div>
              )}

              {/* EARNING PROGRAM MODULE */}
              {activeSheet === 'earning' && (
                <div className="space-y-4">
                  <div className="rounded-lg bg-gradient-to-r from-brand-container to-brand-container-high border p-4">
                    <h4 className="font-sans font-bold text-brand-primary text-xs tracking-tight">Idle Lab Compute Clustering Earning</h4>
                    <p className="mt-1 font-sans text-[11.5px] text-brand-text-muted leading-relaxed">
                      Share idle cores of your high-performance mainframe at the Bern Research Facility back to the AeroFlow research grid nodes. Earn compute credits or tokens in real-time.
                    </p>
                  </div>

                  <div className="border rounded-md p-3 space-y-3 bg-brand-bg">
                    <div className="flex justify-between items-center">
                      <span className="font-sans font-bold uppercase tracking-wide text-[10px] text-brand-text-muted">Program status</span>
                      <button 
                        onClick={() => setHasSharedCluster(!hasSharedCluster)}
                        className={`px-3 py-1 rounded text-[10px] font-bold text-white shadow-xs transition-colors ${hasSharedCluster ? 'bg-emerald-600 hover:bg-emerald-700' : 'bg-brand-primary hover:bg-brand-accent'}`}
                      >
                        {hasSharedCluster ? 'Sharing Online' : 'Opt-In Core Sharing'}
                      </button>
                    </div>

                    {hasSharedCluster && (
                      <div className="space-y-2.5 pt-2 animate-fade-in">
                        <div>
                          <label className="flex justify-between text-xs">
                            <span>Shared mainframe limit</span>
                            <span className="font-mono font-bold text-brand-primary">{clusterStrength}% of multi-nodes</span>
                          </label>
                          <input 
                            type="range" 
                            min="20" 
                            max="100" 
                            step="5" 
                            value={clusterStrength} 
                            onChange={e => setClusterStrength(parseInt(e.target.value))} 
                            className="w-full accent-brand-primary" 
                          />
                        </div>

                        <div className="flex justify-between font-mono text-[10px] border-t border-brand-border pt-2 text-brand-text-muted">
                          <span>Credits Earned This Week:</span>
                          <span className="font-bold text-brand-accent">+240.50 AeroCredits</span>
                        </div>
                      </div>
                    )}
                  </div>
                </div>
              )}
            </div>

            {/* Sheet Footer */}
            <div className="border-t pt-3 mt-4 flex items-center justify-end space-x-2.5">
              <button 
                onClick={() => setActiveSheet(null)}
                className="rounded bg-brand-primary hover:bg-brand-accent text-white px-4 py-1.5 font-sans text-xs font-bold shadow-xs cursor-pointer"
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
