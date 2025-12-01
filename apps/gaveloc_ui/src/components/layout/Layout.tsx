import { ReactNode } from 'react';
import { Titlebar } from '../Titlebar';
import { Sidebar } from './Sidebar';
import { Footer } from './Footer';
import './Layout.css';

export function Layout({ children }: { children: ReactNode }) {
  return (
    <div className="app-container">
      <Titlebar />
      <div className="app-body">
        <Sidebar />
        <main className="app-content">
          {children}
        </main>
        <Footer />
      </div>
    </div>
  );
}
