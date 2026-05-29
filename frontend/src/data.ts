import { Project, UserProfile, SystemLog } from './types';

export const initialUserProfile: UserProfile = {
  name: 'Dr. Aris Thorne',
  role: 'Senior Aerodynamics Lead',
  email: 'a.thorne@aeroflow.tech',
  facility: 'Bern Research Facility',
  group: 'Systems Optimization Group',
  activeSince: 'Active since Nov 2021',
  avatarUrl: 'https://images.unsplash.com/photo-1534528741775-53994a69daeb?q=80&w=256&auto=format&fit=crop', // A polished professional headshot
};

export const initialProjects: Project[] = [
  {
    id: 'proj-delta-7',
    name: 'Project Delta-7',
    version: 'v2.4.1',
    description: 'High-altitude turbulence modeling for prototype regional jet. Optimization of wing-tip vortices.',
    status: 'active',
    teammates: ['JC', 'MI', 'TH', 'DK', 'AR'],
    updatedAt: 'Updated 2h ago',
    parameters: {
      aspectRatio: 9.5,
      reynoldsNumber: 12.4,
      sweepAngle: 28.5,
      machNumber: 0.78,
      liftDragRatio: 18.2,
    },
    suggestions: [
      {
        id: 'sug-1',
        category: 'winglet',
        title: 'Increase winglet cant angle',
        description: 'Increase winglet sweep by 3.5 degrees to minimize wingtip vortices and lower induced drag.',
        impact: '+2.1% L/D ratio',
        applied: false,
        parameterAffected: 'liftDragRatio',
        deltaValue: 0.38,
      },
      {
        id: 'sug-2',
        category: 'mesh',
        title: 'Refine mesh around wing root boundary layer',
        description: 'Increase mesh density at the root blend area to capture secondary separation bubbles.',
        impact: '+1.5% drag reduction',
        applied: false,
        parameterAffected: 'reynoldsNumber',
        deltaValue: 0.8,
      },
      {
        id: 'sug-3',
        category: 'profile',
        title: 'Optimize trailing edge reflex angle',
        description: 'Adjust flaps profile curvature by -0.8% to reduce transonic wave drag.',
        impact: '+0.7% L/D ratio',
        applied: false,
        parameterAffected: 'liftDragRatio',
        deltaValue: 0.13,
      }
    ],
    convergenceData: [
      { iteration: 100, lift: 0.42, drag: 0.045 },
      { iteration: 200, lift: 0.51, drag: 0.038 },
      { iteration: 300, lift: 0.58, drag: 0.034 },
      { iteration: 400, lift: 0.62, drag: 0.032 },
      { iteration: 500, lift: 0.64, drag: 0.031 },
      { iteration: 600, lift: 0.65, drag: 0.0305 },
      { iteration: 700, lift: 0.654, drag: 0.0302 },
      { iteration: 800, lift: 0.655, drag: 0.0301 },
    ],
  },
  {
    id: 'wing-assembly-v4',
    name: 'Wing Assembly v4',
    version: 'STABLE',
    description: 'Structural integrity and thermal expansion simulations for composite airfoil sections.',
    status: 'stable',
    teammates: ['AT', 'RL'],
    updatedAt: 'Updated 5h ago',
    parameters: {
      aspectRatio: 8.2,
      reynoldsNumber: 18.5,
      sweepAngle: 15.0,
      machNumber: 0.55,
      liftDragRatio: 16.8,
    },
    suggestions: [
      {
        id: 'sug-4',
        category: 'surface',
        title: 'Incorporate surface riblets',
        description: 'Introduce microscopic streamwise grooves along 60% chord to sustain laminar boundary layers.',
        impact: '+1.2% viscous drag benefit',
        applied: false,
        parameterAffected: 'liftDragRatio',
        deltaValue: 0.2,
      }
    ],
    convergenceData: [
      { iteration: 100, lift: 0.38, drag: 0.041 },
      { iteration: 200, lift: 0.44, drag: 0.035 },
      { iteration: 300, lift: 0.48, drag: 0.032 },
      { iteration: 400, lift: 0.49, drag: 0.030 },
      { iteration: 500, lift: 0.50, drag: 0.0298 },
    ],
  },
  {
    id: 'turbine-cooling',
    name: 'Turbine Blade Cooling',
    version: 'ARCHIVED',
    description: 'Fluid-structure interaction (FSI) study for next-generation turbine cooling vents.',
    status: 'archived',
    teammates: ['BP'],
    updatedAt: 'Updated 3d ago',
    parameters: {
      aspectRatio: 2.1,
      reynoldsNumber: 4.2,
      sweepAngle: 0.0,
      machNumber: 0.85,
      liftDragRatio: 8.5,
    },
    suggestions: [],
    convergenceData: [
      { iteration: 100, lift: 1.1, drag: 0.18 },
      { iteration: 200, lift: 1.25, drag: 0.16 },
      { iteration: 300, lift: 1.31, drag: 0.155 },
      { iteration: 400, lift: 1.34, drag: 0.153 },
    ],
  }
];

export const initialLogs: SystemLog[] = [
  { id: 'log-1', timestamp: '2026-05-26 00:32:15', project: 'Project Delta-7', event: 'Mesh compilation mesh_refine_v3', status: 'SUCCESS', computeCost: 150 },
  { id: 'log-2', timestamp: '2026-05-26 00:15:44', project: 'Project Delta-7', event: 'Transonic turbulence model init', status: 'CONVERGED', computeCost: 450 },
  { id: 'log-3', timestamp: '2026-05-25 18:50:11', project: 'Wing Assembly v4', event: 'Thermal stress coefficient solve', status: 'SUCCESS', computeCost: 80 },
  { id: 'log-4', timestamp: '2026-05-25 14:12:03', project: 'Project Delta-7', event: 'Navier-Stokes steady-state iteration', status: 'CONVERGED', computeCost: 210 },
  { id: 'log-5', timestamp: '2026-05-24 09:44:56', project: 'Turbine Blade Cooling', event: 'FSI coupling vector boundary check', status: 'SUCCESS', computeCost: 60 },
  { id: 'log-6', timestamp: '2026-05-24 07:11:00', project: 'Wing Assembly v4', event: 'Reynolds Stress turbulence solver', status: 'SUCCESS', computeCost: 200 },
];

export const preconfiguredAirfoils = [
  {
    id: 'naca-4412',
    name: 'NACA 4412 (Symmetric/Subsonic Lift)',
    description: 'Standard cambered passenger jet airfoil designed for high lift at moderate subsonic speeds.',
    svgPath: 'M 10,120 Q 80,60 160,84 Q 240,110 320,120 Q 240,135 160,135 Q 80,135 10,120 Z',
    liftCoef: 0.65,
    dragCoef: 0.032,
    reynolds: 14.2,
    optAoA: 4.5,
  },
  {
    id: 'supercritical-sc02',
    name: 'NASA SC(2)-0714 (Supercritical Airfoil)',
    description: 'Flat upper surface with aft cambered design to delay transonic shockwave drag.',
    svgPath: 'M 10,122 Q 90,85 170,105 Q 250,115 320,120 Q 240,132 160,140 Q 85,138 10,122 Z',
    liftCoef: 0.72,
    dragCoef: 0.026,
    reynolds: 18.5,
    optAoA: 2.1,
  },
  {
    id: 'delta-supersonic',
    name: 'Delta Wing Section (Sharpened Leading Edge)',
    description: 'Ultra-thin flat section with high-angle vortex sheet lift generation for supersonic crafts.',
    svgPath: 'M 10,120 L 160,110 L 320,120 L 160,130 Z',
    liftCoef: 0.48,
    dragCoef: 0.054,
    reynolds: 25.0,
    optAoA: 8.0,
  },
  {
    id: 'high-camber-uav',
    name: 'MH 114 High Camber UAV (Endurance Wing)',
    description: 'Extreme lower surface concavity and tall peak ratio to maximize lift-to-drag at low Reynold numbers.',
    svgPath: 'M 10,120 Q 70,50 160,70 Q 250,90 320,120 Q 250,110 160,105 Q 70,115 10,120 Z',
    liftCoef: 0.94,
    dragCoef: 0.042,
    reynolds: 3.5,
    optAoA: 5.5,
  }
];
