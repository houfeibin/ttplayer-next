import MainPanel from './components/MainPanel/MainPanel';
import SkinProvider from './components/SkinProvider';
import ThemeProvider from './components/ThemeProvider';

function App() {
  return (
    <ThemeProvider>
      <SkinProvider>
        <div style={{
          width: '100%',
          height: '100%',
          display: 'flex',
          flexDirection: 'column',
          overflow: 'hidden',
        }}>
          <MainPanel />
        </div>
      </SkinProvider>
    </ThemeProvider>
  );
}

export default App;
