import React, { useState } from 'react';
import { Bell, Settings, HelpCircle, Search, User, Shield, Key, Sparkles, X, Check } from 'lucide-react';
import { UserProfile } from '../types';

interface HeaderProps {
  user: UserProfile;
  onChangeUser: (updates: Partial<UserProfile>) => void;
  onNavigate: (tab: string) => void;
  activeTab: string;
}

export default function Header(props: HeaderProps) {
  const [showNotifications, setShowNotifications] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [showHelp, setShowHelp] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');

  const notifications = [
    { id: 1, title: 'Simulation Complete', desc: 'Project Delta-7 grid mesh 12M converged.', read: false, time: '10m ago' },
    { id: 2, title: 'AI Recommendation', desc: 'New optimization for Wing Assembly v4 trailing edge.', read: false, time: '1h ago' },
    { id: 3, title: 'Plan Status Alert', desc: 'You have consumed 78% of your compute quota.', read: true, time: '1d ago' },
  ];

  return (
    <header id="aeroflow-header" className="sticky top-0 z-40 flex items-center justify-between border-b border-brand-border bg-white px-6 py-3 shadow-xs">
      {/* Brand Logo and Navigation items */}
      <div id="header-brand-section" className="flex items-center space-x-8">
        <div 
          onClick={() => props.onNavigate('projects')}
          className="flex cursor-pointer items-center space-x-2"
        >
          <div className="flex h-8 w-8 items-center justify-center rounded-md bg-brand-primary text-white font-bold shadow-xs">
            AF
          </div>
          <span className="font-sans text-xl font-bold tracking-tight text-brand-primary">
            AeroFlow
          </span>
        </div>

        {/* Dynamic Search Bar */}
        <div id="header-search-bar" className="relative hidden w-64 md:block">
          <span className="absolute inset-y-0 left-0 flex items-center pl-3 text-brand-text-muted">
            <Search className="h-4 w-4" />
          </span>
          <input
            type="text"
            placeholder="Search projects, simulations, logs..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full rounded-md border border-brand-border bg-brand-bg py-1.5 pl-9 pr-4 font-sans text-xs text-brand-text outline-hidden transition-all placeholder:text-brand-text-muted focus:border-brand-primary focus:ring-1 focus:ring-brand-primary"
          />
        </div>
      </div>

      {/* Global Actions Bar */}
      <div id="header-actions" className="flex items-center space-x-4">
        {/* Notifications Icon with Badge */}
        <div className="relative">
          <button
            id="notification-bell-btn"
            onClick={() => {
              setShowNotifications(!showNotifications);
              setShowSettings(false);
              setShowHelp(false);
            }}
            className="relative rounded-full p-1.5 text-brand-text-muted hover:bg-brand-bg hover:text-brand-primary transition-colors focus:outline-hidden"
          >
            <Bell className="h-5 w-5" />
            <span className="absolute top-1 right-1 flex h-2 w-2">
              <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-brand-primary opacity-75"></span>
              <span className="relative inline-flex h-2 w-2 rounded-full bg-brand-accent"></span>
            </span>
          </button>

          {/* Notifications Dropdown */}
          {showNotifications && (
            <div id="notifications-box" className="absolute right-0 mt-3 w-80 rounded-lg border border-brand-border bg-white p-2 shadow-lg animate-fade-in">
              <div className="flex items-center justify-between border-b border-brand-container-low px-3 py-2">
                <span className="font-sans text-xs font-semibold text-brand-text">Recent Notifications</span>
                <span className="font-mono text-[10px] text-brand-accent cursor-pointer hover:underline">Mark all read</span>
              </div>
              <div className="max-h-64 overflow-y-auto pt-1">
                {notifications.map((notif) => (
                  <div key={notif.id} className={`p-2.5 rounded-md hover:bg-brand-container-low transition-colors cursor-pointer border-b border-brand-bg last:border-b-0 ${!notif.read ? 'bg-brand-container-low/40' : ''}`}>
                    <div className="flex items-start justify-between">
                      <span className="font-sans text-xs font-semibold text-brand-text">{notif.title}</span>
                      <span className="font-mono text-[10px] text-brand-text-muted">{notif.time}</span>
                    </div>
                    <p className="mt-0.5 font-sans text-[11px] text-brand-text-muted">{notif.desc}</p>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>

        {/* Settings Panel */}
        <div className="relative">
          <button
            id="settings-tool-btn"
            onClick={() => {
              setShowSettings(!showSettings);
              setShowNotifications(false);
              setShowHelp(false);
            }}
            className="rounded-full p-1.5 text-brand-text-muted hover:bg-brand-bg hover:text-brand-primary transition-colors focus:outline-hidden"
          >
            <Settings className="h-5 w-5" />
          </button>

          {showSettings && (
            <div id="settings-box" className="absolute right-0 mt-3 w-72 rounded-lg border border-brand-border bg-white p-3 shadow-lg animate-fade-in">
              <h3 className="font-sans text-xs font-bold text-brand-text border-b border-brand-bg pb-1.5 mb-2">System Configurations</h3>
              <div className="space-y-2.5">
                <div className="flex items-center justify-between text-xs">
                  <span className="text-brand-text-muted flex items-center gap-1.5"><Shield className="h-3.5 w-3.5" /> High-Accuracy Solvers</span>
                  <input type="checkbox" defaultChecked className="accent-brand-primary scale-90" />
                </div>
                <div className="flex items-center justify-between text-xs">
                  <span className="text-brand-text-muted flex items-center gap-1.5"><Sparkles className="h-3.5 w-3.5" /> Generative CFD Mesh</span>
                  <input type="checkbox" defaultChecked className="accent-brand-primary scale-90" />
                </div>
                <div className="flex items-center justify-between text-xs">
                  <span className="text-brand-text-muted flex items-center gap-1.5"><Key className="h-3.5 w-3.5" /> Secure Hardware Keys</span>
                  <span className="font-mono text-[10px] bg-brand-container px-1.5 py-0.5 rounded text-brand-primary">Active</span>
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Help Panel */}
        <div className="relative">
          <button
            id="help-center-btn"
            onClick={() => {
              setShowHelp(!showHelp);
              setShowNotifications(false);
              setShowSettings(false);
            }}
            className="rounded-full p-1.5 text-brand-text-muted hover:bg-brand-bg hover:text-brand-primary transition-colors focus:outline-hidden"
          >
            <HelpCircle className="h-5 w-5" />
          </button>

          {showHelp && (
            <div id="help-box" className="absolute right-0 mt-3 w-72 rounded-lg border border-brand-border bg-white p-3 shadow-lg animate-fade-in">
              <h3 className="font-sans text-xs font-bold text-brand-text border-b border-brand-bg pb-1.5 mb-2">AeroFlow Help Center</h3>
              <ul className="space-y-2 text-xs text-brand-text-muted">
                <li>
                  <a href="#quickstart" onClick={() => { props.onNavigate('doc'); setShowHelp(false); }} className="hover:text-brand-primary hover:underline block font-sans">
                    • Aerodynamics Formulas Cheat Sheet
                  </a>
                </li>
                <li>
                  <a href="#cfd" onClick={() => { props.onNavigate('visualize'); setShowHelp(false); }} className="hover:text-brand-primary hover:underline block font-sans">
                    • How to run Navier-Stokes CFD simulations
                  </a>
                </li>
                <li>
                  <span className="block font-sans text-[10px] text-brand-text-muted mt-2 border-t pt-1.5">
                    Terminal Support: support@aeroflow.tech
                  </span>
                </li>
              </ul>
            </div>
          )}
        </div>

        {/* User Identity Info */}
        <div className="flex items-center space-x-2.5 border-l border-brand-border pl-3">
          <div className="hidden flex-col items-end sm:flex">
            <span className="font-sans text-[11px] font-semibold text-brand-text leading-tight">{props.user.name}</span>
            <span className="font-mono text-[9px] text-brand-accent uppercase leading-none">{props.user.role}</span>
          </div>
          <img
            src={props.user.avatarUrl}
            alt={props.user.name}
            referrerPolicy="no-referrer"
            className="h-8 w-8 rounded-full border border-brand-border object-cover ring-2 ring-brand-container-low"
          />
        </div>
      </div>
    </header>
  );
}
