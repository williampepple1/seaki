import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Wifi, Upload, Download, RefreshCw, CheckCircle, AlertCircle, Shield, FileText, Clock, X } from 'lucide-react';

interface Device {
  id: string;
  name: string;
  ip: string;
  port: number;
  last_seen: string;
}


interface IncomingConnection {
  id: string;
  device_name: string;
  device_ip: string;
  timestamp: string;
}

interface IncomingFile {
  id: string;
  file_name: string;
  file_size: number;
  sender_name: string;
  sender_ip: string;
  timestamp: string;
}

function App() {
  const [devices, setDevices] = useState<Device[]>([]);
  const [selectedDevice, setSelectedDevice] = useState<Device | null>(null);
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [isServerRunning, setIsServerRunning] = useState(false);
  const [isDiscovering, setIsDiscovering] = useState(false);
  const [isSending, setIsSending] = useState(false);
  const [localIp, setLocalIp] = useState<string>('');
  const [message, setMessage] = useState<{ type: 'success' | 'error' | 'info'; text: string } | null>(null);
  const [progress, setProgress] = useState(0);
  const [pendingConnections, setPendingConnections] = useState<IncomingConnection[]>([]);
  const [pendingFiles, setPendingFiles] = useState<IncomingFile[]>([]);
  const [activeTab, setActiveTab] = useState<'devices' | 'connections' | 'files'>('devices');

  useEffect(() => {
    getLocalIp();
    discoverDevices();
    loadPendingData();
    
    // Poll for pending connections and files
    const interval = setInterval(() => {
      loadPendingData();
    }, 2000);
    
    return () => clearInterval(interval);
  }, []);

  const getLocalIp = async () => {
    try {
      const ip = await invoke<string>('get_local_ip');
      setLocalIp(ip);
    } catch (error) {
      console.error('Failed to get local IP:', error);
    }
  };

  const startServer = async () => {
    try {
      const result = await invoke<string>('start_server');
      setIsServerRunning(true);
      setMessage({ type: 'success', text: result });
    } catch (error) {
      setMessage({ type: 'error', text: `Failed to start server: ${error}`});
    }
  };

  const stopServer = async () => {
    try {
      const result = await invoke<string>('stop_server');
      setIsServerRunning(false);
      setMessage({ type: 'info', text: result });
    } catch (error) {
      setMessage({ type: 'error', text: `Failed to stop server: ${error}`});
    }
  };

  const discoverDevices = async () => {
    setIsDiscovering(true);
    try {
      const discoveredDevices = await invoke<Device[]>('discover_devices');
      setDevices(discoveredDevices);
      setMessage({ type: 'info', text: `Found ${discoveredDevices.length} devices` });
    } catch (error) {
      setMessage({ type: 'error', text: `Failed to discover devices: ${error}`});
    } finally {
      setIsDiscovering(false);
    }
  };


  const sendFile = async () => {
    if (!selectedDevice || !selectedFile) {
      setMessage({ type: 'error', text: 'Please select a device and file' });
      return;
    }

    setIsSending(true);
    setProgress(0);

    try {
      // Simulate progress
      const progressInterval = setInterval(() => {
        setProgress(prev => Math.min(prev + 10, 90));
      }, 200);

      const result = await invoke<string>('send_file', {
        deviceIp: selectedDevice.ip,
        filePath: selectedFile.name
      });

      clearInterval(progressInterval);
      setProgress(100);
      setMessage({ type: 'success', text: result });
    } catch (error) {
      setMessage({ type: 'error', text: `Failed to send file: ${error}`});
    } finally {
      setIsSending(false);
      setTimeout(() => setProgress(0), 2000);
    }
  };

  const loadPendingData = async () => {
    try {
      const [connections, files] = await Promise.all([
        invoke<IncomingConnection[]>('get_pending_connections'),
        invoke<IncomingFile[]>('get_pending_files')
      ]);
      setPendingConnections(connections);
      setPendingFiles(files);
    } catch (error) {
      console.error('Failed to load pending data:', error);
    }
  };

  const approveConnection = async (connectionId: string, approved: boolean) => {
    try {
      const result = await invoke<string>('approve_connection', {
        connectionId,
        approved
      });
      setMessage({ type: approved ? 'success' : 'info', text: result });
      loadPendingData();
      if (approved) {
        discoverDevices(); // Refresh device list
      }
    } catch (error) {
      setMessage({ type: 'error', text: `Failed to handle connection: ${error}`});
    }
  };

  const approveFileTransfer = async (fileId: string, approved: boolean) => {
    try {
      let savePath: string | undefined;
      
      if (approved) {
        // For now, use a default save path
        savePath = "./downloads/";
      }

      const result = await invoke<string>('approve_file_transfer', {
        fileId,
        approved,
        savePath
      });
      setMessage({ type: approved ? 'success' : 'info', text: result });
      loadPendingData();
    } catch (error) {
      setMessage({ type: 'error', text: `Failed to handle file transfer: ${error}`});
    }
  };

  const formatFileSize = (bytes: number) => {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  const formatTimestamp = (timestamp: string) => {
    return new Date(timestamp).toLocaleString();
  };

  return (
    <div className="container">
      <div className="header">
        <h1>Seaki</h1>
        <p>Local File Sharing - Send files without internet</p>
      </div>

      {message && (
        <div className={`message ${message.type}`}>
          {message.text}
        </div>
      )}

      <div className="card">
        <div className="status-bar">
          <div className="status-indicator">
            <div className={`status-dot ${isServerRunning ? '' : 'offline'}`}></div>
            <span>{isServerRunning ? 'Server Running' : 'Server Offline'}</span>
            {localIp && <span>({localIp})</span>}
          </div>
          <div className="buttons">
            {!isServerRunning ? (
              <button className="btn btn-primary" onClick={startServer}>
                <Wifi size={16} />
                Start Server
              </button>
            ) : (
              <button className="btn btn-secondary" onClick={stopServer}>
                Stop Server
              </button>
            )}
            <button 
              className="btn btn-secondary" 
              onClick={discoverDevices}
              disabled={isDiscovering}
            >
              <RefreshCw size={16} className={isDiscovering ? 'loading' : ''} />
              {isDiscovering ? 'Discovering...' : 'Refresh'}
            </button>
          </div>
        </div>

        {/* Tab Navigation */}
        <div className="tab-navigation">
          <button 
            className={`tab-button ${activeTab === 'devices' ? 'active' : ''}`}
            onClick={() => setActiveTab('devices')}
          >
            <Wifi size={16} />
            Devices ({devices.length})
          </button>
          <button 
            className={`tab-button ${activeTab === 'connections' ? 'active' : ''}`}
            onClick={() => setActiveTab('connections')}
          >
            <Shield size={16} />
            Connections ({pendingConnections.length})
          </button>
          <button 
            className={`tab-button ${activeTab === 'files' ? 'active' : ''}`}
            onClick={() => setActiveTab('files')}
          >
            <FileText size={16} />
            Files ({pendingFiles.length})
          </button>
        </div>

        {/* Devices Tab */}
        {activeTab === 'devices' && (
          <>
            <div className="device-grid">
              {devices.map((device) => (
                <div
                  key={device.id}
                  className={`device-card ${selectedDevice?.id === device.id ? 'selected' : ''}`}
                  onClick={() => setSelectedDevice(device)}
                >
                  <div className="device-name">{device.name}</div>
                  <div className="device-ip">{device.ip}:{device.port}</div>
                  <div className="device-status">
                    <CheckCircle size={14} />
                    Online
                  </div>
                </div>
              ))}
            </div>

            {devices.length === 0 && (
              <div style={{ textAlign: 'center', padding: '40px', color: '#718096' }}>
                <AlertCircle size={48} style={{ marginBottom: '16px', opacity: 0.5 }} />
                <p>No devices found. Make sure other devices are running Seaki and connected to the same WiFi.</p>
              </div>
            )}
          </>
        )}

        {/* Connections Tab */}
        {activeTab === 'connections' && (
          <div className="pending-list">
            {pendingConnections.length === 0 ? (
              <div style={{ textAlign: 'center', padding: '40px', color: '#718096' }}>
                <Shield size={48} style={{ marginBottom: '16px', opacity: 0.5 }} />
                <p>No pending connection requests</p>
              </div>
            ) : (
              pendingConnections.map((connection) => (
                <div key={connection.id} className="pending-item">
                  <div className="pending-info">
                    <div className="pending-title">{connection.device_name}</div>
                    <div className="pending-details">
                      <span>{connection.device_ip}</span>
                      <span className="pending-time">
                        <Clock size={12} />
                        {formatTimestamp(connection.timestamp)}
                      </span>
                    </div>
                  </div>
                  <div className="pending-actions">
                    <button 
                      className="btn btn-success"
                      onClick={() => approveConnection(connection.id, true)}
                    >
                      <CheckCircle size={14} />
                      Accept
                    </button>
                    <button 
                      className="btn btn-secondary"
                      onClick={() => approveConnection(connection.id, false)}
                    >
                      <X size={14} />
                      Reject
                    </button>
                  </div>
                </div>
              ))
            )}
          </div>
        )}

        {/* Files Tab */}
        {activeTab === 'files' && (
          <div className="pending-list">
            {pendingFiles.length === 0 ? (
              <div style={{ textAlign: 'center', padding: '40px', color: '#718096' }}>
                <FileText size={48} style={{ marginBottom: '16px', opacity: 0.5 }} />
                <p>No pending file transfers</p>
              </div>
            ) : (
              pendingFiles.map((file) => (
                <div key={file.id} className="pending-item">
                  <div className="pending-info">
                    <div className="pending-title">{file.file_name}</div>
                    <div className="pending-details">
                      <span>{formatFileSize(file.file_size)} from {file.sender_name}</span>
                      <span className="pending-time">
                        <Clock size={12} />
                        {formatTimestamp(file.timestamp)}
                      </span>
                    </div>
                  </div>
                  <div className="pending-actions">
                    <button 
                      className="btn btn-success"
                      onClick={() => approveFileTransfer(file.id, true)}
                    >
                      <Download size={14} />
                      Accept
                    </button>
                    <button 
                      className="btn btn-secondary"
                      onClick={() => approveFileTransfer(file.id, false)}
                    >
                      <X size={14} />
                      Reject
                    </button>
                  </div>
                </div>
              ))
            )}
          </div>
        )}
      </div>

      {selectedDevice && (
        <div className="card">
          <h3 style={{ marginBottom: '20px', color: '#2d3748' }}>
            Send File to {selectedDevice.name}
          </h3>
          
          <div className="file-section">
            <div className="file-input">
              <input
                type="file"
                id="file-input"
                onChange={(e) => {
                  const file = e.target.files?.[0];
                  if (file) {
                    setSelectedFile(file);
                    setMessage({ type: 'success', text: `Selected file: ${file.name}` });
                  }
                }}
                style={{ display: 'none' }}
              />
              <label htmlFor="file-input" className="file-button">
                <Upload size={16} />
                Choose File
              </label>
              <span style={{ color: '#718096' }}>
                {selectedFile ? selectedFile.name : 'No file selected'}
              </span>
            </div>

            {selectedFile && (
              <div className="selected-file">
                <div className="file-info">
                  <span className="file-name">{selectedFile.name}</span>
                  <span className="file-size">{formatFileSize(selectedFile.size)}</span>
                </div>
                {isSending && (
                  <div className="progress-bar">
                    <div 
                      className="progress-fill" 
                      style={{ width: `${progress}%` }}
                    ></div>
                  </div>
                )}
              </div>
            )}

            <div className="buttons">
              <button
                className="btn btn-success"
                onClick={sendFile}
                disabled={!selectedFile || isSending}
              >
                {isSending ? (
                  <>
                    <div className="loading"></div>
                    Sending...
                  </>
                ) : (
                  <>
                    <Download size={16} />
                    Send File
                  </>
                )}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default App;
