export interface User {
    id: number;
    name: string;
    github_nickname?: string;
    github_avatar?: string;
    twitter_nickname?: string;
    twitter_avatar?: string;
}

export type PageProps<T extends Record<string, unknown> = Record<string, unknown>> = T & {
    auth: {
        user: User;
    };
    flash: {
        message: string | null;
    }
};
