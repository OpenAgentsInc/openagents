import AuthenticatedLayout from '@/Layouts/AuthenticatedLayout';
import { Head } from '@inertiajs/react';
import { PageProps } from '@/types';
import { Button } from '@/Components/ui/button';
import axios from 'axios';

export default function Dashboard({ auth }: PageProps) {
    const triggerRun = () => {
        axios.post('/faerie-run').then((response) => response.data)
            .then((data) => {
                console.log(data)
            })
            .catch((error) => {
                console.log(error)
            })
    }
    return (
        <AuthenticatedLayout
            user={auth.user}
            header={<h2 className="font-semibold text-xl text-gray-800 dark:text-gray-200 leading-tight">Dashboard</h2>}
        >
            <Head title="Dashboard" />

            <div className="py-12">
                <div className="max-w-7xl mx-auto sm:px-6 lg:px-8">
                    <div className="min-h-[400px] bg-white dark:bg-gray-800 overflow-hidden shadow-sm sm:rounded-lg">
                        {auth.user?.github_nickname === 'AtlantisPleb' ? (
                            <div className="p-6">
                                <Button onClick={triggerRun}>Go Faerie</Button>
                            </div>
                        ) : <div className="p-6 text-gray-900 dark:text-gray-100">Nothing to do here!</div>
                        }
                    </div>
                </div>
            </div>
        </AuthenticatedLayout>
    );
}
