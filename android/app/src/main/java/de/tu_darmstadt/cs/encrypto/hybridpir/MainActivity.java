package de.tu_darmstadt.cs.encrypto.hybridpir;

import java.util.Arrays;

import androidx.appcompat.app.AppCompatActivity;

import android.os.Bundle;
import android.widget.TextView;

public class MainActivity extends AppCompatActivity {

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        setContentView(R.layout.activity_main);

        RustInterface i = new RustInterface();

        String targets[] = {
            "192.168.178.20:7000",
            "192.168.178.20:7001"
        };

        i.benchmarkPEM();

        TextView tv = (TextView)findViewById(R.id.textView);
        tv.setText("Done!");
    }

    static {
        System.loadLibrary("hybridpir");
    }
}
