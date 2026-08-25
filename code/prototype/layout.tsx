import type { Metadata } from 'next';
import './globals.css';

export const metadata: Metadata = {
  title: 'Caretaker Lab — human growth calculator',
  description: 'Import a The Last Caretaker save and calculate minimum-waste recipes.',
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return <html lang="en"><body>{children}</body></html>;
}
