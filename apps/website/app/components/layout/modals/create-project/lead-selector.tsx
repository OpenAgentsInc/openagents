import { Button } from '@/components/ui/button';
import { Avatar, AvatarFallback, AvatarImage } from '@/components/ui/avatar';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { useEffect, useState } from 'react';
import { User } from 'lucide-react';
import { getDb } from '@/lib/db/project-helpers';

interface UserData {
  id: string;
  name: string;
  email: string;
  image: string | null;
}

interface LeadSelectorProps {
  leadId: string | null;
  onChange: (leadId: string | null) => void;
}

export function LeadSelector({ leadId, onChange }: LeadSelectorProps) {
  const [users, setUsers] = useState<UserData[]>([]);
  const [selectedUser, setSelectedUser] = useState<UserData | null>(null);
  const [loading, setLoading] = useState(true);

  // Fetch users on mount
  useEffect(() => {
    async function fetchUsers() {
      try {
        const db = getDb();
        const results = await db
          .selectFrom('user')
          .select(['id', 'name', 'email', 'image'])
          .orderBy('name')
          .execute();
        
        setUsers(results);
        
        if (leadId) {
          const found = results.find(u => u.id === leadId);
          if (found) setSelectedUser(found);
        }
        
        setLoading(false);
      } catch (error) {
        console.error('Error fetching users:', error);
        setLoading(false);
      }
    }
    
    fetchUsers();
  }, [leadId]);

  const handleUserChange = (newUserId: string) => {
    if (newUserId === 'none') {
      setSelectedUser(null);
      onChange(null);
      return;
    }
    
    const user = users.find(u => u.id === newUserId);
    if (user) {
      setSelectedUser(user);
      onChange(newUserId);
    }
  };

  const getInitials = (name: string) => {
    return name
      .split(' ')
      .map(n => n[0])
      .join('')
      .toUpperCase()
      .substring(0, 2);
  };

  if (loading) {
    return (
      <Button size="sm" variant="outline" className="gap-1.5" disabled>
        Loading...
      </Button>
    );
  }

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button size="sm" variant="outline" className="gap-1.5">
          {selectedUser ? (
            <>
              <Avatar className="size-5">
                <AvatarImage src={selectedUser.image || ''} />
                <AvatarFallback>{getInitials(selectedUser.name)}</AvatarFallback>
              </Avatar>
              <span>{selectedUser.name}</span>
            </>
          ) : (
            <>
              <User className="size-4" />
              <span>Lead</span>
            </>
          )}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent className="w-56">
        <DropdownMenuRadioGroup value={leadId || 'none'} onValueChange={handleUserChange}>
          <DropdownMenuRadioItem value="none">
            <div className="flex items-center gap-2">
              <User className="size-4" />
              <span>No lead</span>
            </div>
          </DropdownMenuRadioItem>
          
          {users.map((user) => (
            <DropdownMenuRadioItem key={user.id} value={user.id}>
              <div className="flex items-center gap-2">
                <Avatar className="size-5">
                  <AvatarImage src={user.image || ''} />
                  <AvatarFallback>{getInitials(user.name)}</AvatarFallback>
                </Avatar>
                <span>{user.name}</span>
              </div>
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}