import { useLoaderData } from 'react-router';
import TeamLine from './team-line';

interface Team {
  id: string;
  name: string;
  key: string;
  icon: string;
  color: string;
  joined: boolean;
  memberCount: number;
  projectCount: number;
}

interface LoaderData {
  teams: Team[];
  error?: string;
}

export function Teams() {
  const { teams, error } = useLoaderData() as LoaderData;

  if (error) {
    return (
      <div className="w-full p-8 text-center">
        <p className="text-red-500">Error loading teams: {error}</p>
      </div>
    );
  }

  if (!teams || teams.length === 0) {
    return (
      <div className="w-full p-8 text-center">
        <p className="text-muted-foreground">No teams found. Create a team to get started.</p>
      </div>
    );
  }

  return (
    <div className="w-full">
      <div className="bg-container px-6 py-1.5 text-sm flex items-center text-muted-foreground border-b sticky top-0 z-10">
        <div className="w-[70%] sm:w-[50%] md:w-[45%] lg:w-[40%]">Name</div>
        <div className="hidden sm:block sm:w-[20%] md:w-[15%]">Membership</div>
        <div className="hidden sm:block sm:w-[20%] md:w-[15%]">Identifier</div>
        <div className="w-[30%] sm:w-[20%] md:w-[15%]">Members</div>
        <div className="hidden sm:block sm:w-[20%] md:w-[15%]">Projects</div>
      </div>

      <div className="w-full">
        {teams.map((team) => (
          <TeamLine key={team.id} team={team} />
        ))}
      </div>
    </div>
  );
}

export default Teams;