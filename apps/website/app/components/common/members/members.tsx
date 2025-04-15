'use client';

import { users } from '@/mock-data/users';
import MemberLine from './member-line';

export default function Members() {
   return (
      <div className="w-full">
         <div className="bg-container px-6 py-1.5 text-sm flex items-center text-muted-foreground border-b sticky top-0 z-10">
            <div className="w-[70%] md:w-[60%] lg:w-[55%]">Name</div>
            <div className="w-[30%] md:w-[20%] lg:w-[15%]">Status</div>
            <div className="hidden lg:block w-[15%]">Joined</div>
            <div className="w-[30%] hidden md:block md:w-[20%] lg:w-[15%]">Teams</div>
         </div>

         <div className="w-full">
            {users.map((user) => (
               <MemberLine key={user.id} user={user} />
            ))}
         </div>
      </div>
   );
}
