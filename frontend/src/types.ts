export interface AerodynamicParameters {
  aspectRatio: number;
  reynoldsNumber: number; // in millions
  sweepAngle: number; // degrees
  machNumber: number;
  liftDragRatio: number;
}

export interface AISuggestion {
  id: string;
  category: 'mesh' | 'winglet' | 'surface' | 'profile';
  title: string;
  description: string;
  impact: string;
  applied: boolean;
  parameterAffected: keyof AerodynamicParameters;
  deltaValue: number;
}

export interface Project {
  id: string;
  name: string;
  version: string;
  description: string;
  status: 'active' | 'stable' | 'archived';
  teammates: string[];
  updatedAt: string;
  parameters: AerodynamicParameters;
  suggestions: AISuggestion[];
  convergenceData: { iteration: number; lift: number; drag: number }[];
  cadFile?: {
    name: string;
    size: string;
    type: string;
    uploadedAt: string;
  };
}

export interface UserProfile {
  name: string;
  role: string;
  email: string;
  facility: string;
  group: string;
  activeSince: string;
  avatarUrl: string;
}

export interface SystemLog {
  id: string;
  timestamp: string;
  project: string;
  event: string;
  status: 'SUCCESS' | 'RUNNING' | 'CONVERGED' | 'COMPILING';
  computeCost: number; // tokens or hours
}

export function calcLiftDragRatio(aspectRatio: number, machNumber: number, sweepAngle: number): number {
  return parseFloat(((aspectRatio * 1.5) + (machNumber * 4) - (sweepAngle * 0.04)).toFixed(1));
}
