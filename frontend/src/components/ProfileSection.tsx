import React, { useState } from 'react';
import { Mail, MapPin, Briefcase, Calendar, Edit3, X, Check, Camera } from 'lucide-react';
import { UserProfile } from '../types';

interface ProfileSectionProps {
  user: UserProfile;
  onUpdateUser: (updatedUser: UserProfile) => void;
}

export default function ProfileSection(props: ProfileSectionProps) {
  const [isEditing, setIsEditing] = useState(false);
  const [editedName, setEditedName] = useState(props.user.name);
  const [editedRole, setEditedRole] = useState(props.user.role);
  const [editedEmail, setEditedEmail] = useState(props.user.email);
  const [editedFacility, setEditedFacility] = useState(props.user.facility);
  const [editedGroup, setEditedGroup] = useState(props.user.group);
  const [editedAvatar, setEditedAvatar] = useState(props.user.avatarUrl);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    props.onUpdateUser({
      name: editedName,
      role: editedRole,
      email: editedEmail,
      facility: editedFacility,
      group: editedGroup,
      avatarUrl: editedAvatar,
      activeSince: props.user.activeSince,
    });
    setIsEditing(false);
  };

  const resetForm = () => {
    setEditedName(props.user.name);
    setEditedRole(props.user.role);
    setEditedEmail(props.user.email);
    setEditedFacility(props.user.facility);
    setEditedGroup(props.user.group);
    setEditedAvatar(props.user.avatarUrl);
    setIsEditing(false);
  };

  return (
    <div id="profile-container" className="relative rounded-lg border border-brand-border bg-white p-5">
      <div className="flex flex-col gap-5 sm:flex-row sm:items-center sm:justify-between">
        {/* Profile Card Info Block */}
        <div className="flex flex-col gap-4 sm:flex-row sm:items-center">
          {/* Avatar Container with Senior Check Badge */}
          <div className="relative self-start sm:self-center">
            <img
              src={props.user.avatarUrl}
              alt={props.user.name}
              referrerPolicy="no-referrer"
              className="h-20 w-20 rounded-lg object-cover border border-brand-border"
            />
            <div className="absolute -bottom-1 -right-1 flex h-5 w-5 items-center justify-center rounded-full bg-brand-primary border-2 border-white text-white">
              {/* Senior engineer mark */}
              <Check className="h-3 w-3 stroke-[3]" />
            </div>
          </div>

          {/* Details Column */}
          <div className="space-y-1">
            <div className="flex items-baseline space-x-2">
              <h2 className="font-sans text-xl font-bold tracking-tight text-brand-text">
                {props.user.name}
              </h2>
            </div>
            <p className="font-sans text-xs font-semibold text-brand-text-muted">
              {props.user.role}
            </p>

            {/* Grid of details */}
            <div className="mt-3 grid grid-cols-1 gap-x-6 gap-y-1.5 pt-1.5 text-xs text-brand-text-muted md:grid-cols-2">
              <div className="flex items-center space-x-2">
                <Mail className="h-3.5 w-3.5 text-brand-text-muted" />
                <span className="font-mono">{props.user.email}</span>
              </div>
              <div className="flex items-center space-x-2">
                <MapPin className="h-3.5 w-3.5 text-brand-text-muted" />
                <span>{props.user.facility}</span>
              </div>
              <div className="flex items-center space-x-2">
                <Briefcase className="h-3.5 w-3.5 text-brand-text-muted" />
                <span>{props.user.group}</span>
              </div>
              <div className="flex items-center space-x-2">
                <Calendar className="h-3.5 w-3.5 text-brand-text-muted" />
                <span>{props.user.activeSince}</span>
              </div>
            </div>
          </div>
        </div>

        {/* Edit Button */}
        <button
          onClick={() => setIsEditing(true)}
          className="flex items-center space-x-1.5 self-start rounded-md border border-brand-border bg-white px-3 py-1.5 font-sans text-xs font-semibold text-brand-text-muted hover:bg-brand-bg hover:text-brand-text transition-colors sm:self-center"
        >
          <Edit3 className="h-3.5 w-3.5" />
          <span>Edit Profile</span>
        </button>
      </div>

      {/* Profile Edit Overlay Modal */}
      {isEditing && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-xs p-4 animate-fade-in">
          <div className="w-full max-w-md rounded-lg border border-brand-border bg-white p-5 shadow-2xl animate-slide-up">
            <div className="flex items-center justify-between border-b border-brand-bg pb-3">
              <h3 className="font-sans text-xs font-bold text-brand-text uppercase tracking-wider">Modify Professional Profile</h3>
              <button onClick={resetForm} className="rounded-md p-1 hover:bg-brand-bg">
                <X className="h-4 w-4 text-brand-text-muted" />
              </button>
            </div>

            <form onSubmit={handleSubmit} className="mt-4 space-y-3">
              <div>
                <label className="block font-sans text-[11px] font-bold text-brand-text uppercase tracking-wide">Avatar Image URL</label>
                <div className="mt-1 flex items-center space-x-2">
                  <input
                    type="text"
                    value={editedAvatar}
                    onChange={(e) => setEditedAvatar(e.target.value)}
                    className="w-full rounded-md border border-brand-border bg-brand-bg px-3 py-1.5 font-sans text-xs text-brand-text outline-hidden"
                  />
                  <button 
                    type="button" 
                    onClick={() => setEditedAvatar('https://images.unsplash.com/photo-1507003211169-0a1dd7228f2d?q=80&w=256&auto=format&fit=crop')}
                    className="rounded-md border border-brand-border bg-white p-1.5 hover:bg-brand-bg text-[10px]"
                    title="Use Male Headshot"
                  >
                    Male
                  </button>
                  <button 
                    type="button" 
                    onClick={() => setEditedAvatar('https://images.unsplash.com/photo-1544005313-94ddf0286df2?q=80&w=256&auto=format&fit=crop')}
                    className="rounded-md border border-brand-border bg-white p-1.5 hover:bg-brand-bg text-[10px]"
                    title="Use Female Headshot"
                  >
                    Female
                  </button>
                </div>
              </div>

              <div>
                <label className="block font-sans text-[11px] font-bold text-brand-text uppercase tracking-wide">Professional Name</label>
                <input
                  type="text"
                  required
                  value={editedName}
                  onChange={(e) => setEditedName(e.target.value)}
                  className="mt-1 w-full rounded-md border border-brand-border bg-brand-bg px-3 py-1.5 font-sans text-xs text-brand-text outline-hidden focus:border-brand-primary focus:ring-1 focus:ring-brand-primary"
                />
              </div>

              <div>
                <label className="block font-sans text-[11px] font-bold text-brand-text uppercase tracking-wide">System Role Title</label>
                <input
                  type="text"
                  value={editedRole}
                  onChange={(e) => setEditedRole(e.target.value)}
                  className="mt-1 w-full rounded-md border border-brand-border bg-brand-bg px-3 py-1.5 font-sans text-xs text-brand-text outline-hidden focus:border-brand-primary focus:ring-1 focus:ring-brand-primary"
                />
              </div>

              <div>
                <label className="block font-sans text-[11px] font-bold text-brand-text uppercase tracking-wide">Email</label>
                <input
                  type="email"
                  required
                  value={editedEmail}
                  onChange={(e) => setEditedEmail(e.target.value)}
                  className="mt-1 w-full rounded-md border border-brand-border bg-brand-bg px-3 py-1.5 font-sans text-xs text-brand-text outline-hidden focus:border-brand-primary"
                />
              </div>

              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block font-sans text-[11px] font-bold text-brand-text uppercase tracking-wide">Research Facility</label>
                  <input
                    type="text"
                    value={editedFacility}
                    onChange={(e) => setEditedFacility(e.target.value)}
                    className="mt-1 w-full rounded-md border border-brand-border bg-brand-bg px-3 py-1.5 font-sans text-xs text-brand-text outline-hidden"
                  />
                </div>
                <div>
                  <label className="block font-sans text-[11px] font-bold text-brand-text uppercase tracking-wide">Optimization Group</label>
                  <input
                    type="text"
                    value={editedGroup}
                    onChange={(e) => setEditedGroup(e.target.value)}
                    className="mt-1 w-full rounded-md border border-brand-border bg-brand-bg px-3 py-1.5 font-sans text-xs text-brand-text outline-hidden"
                  />
                </div>
              </div>

              <div className="mt-5 flex items-center justify-end space-x-3 border-t border-brand-bg pt-3">
                <button
                  type="button"
                  onClick={resetForm}
                  className="rounded-md border border-brand-border bg-white px-3.5 py-1.5 font-sans text-xs font-semibold text-brand-text-muted hover:bg-brand-bg"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  className="rounded-md bg-brand-primary px-4 py-1.5 font-sans text-xs font-bold text-white shadow-xs hover:bg-brand-accent"
                >
                  Save Changes
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}
